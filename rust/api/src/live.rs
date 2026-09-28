//! One server-owned Alpaca connection. Browser clients receive bounded snapshots;
//! the append-only journal preserves provider events and local receipt times.
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, sync::Arc, time::Duration};
use tokio::sync::{watch, RwLock};
mod journal;
use journal::Journal;
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{protocol::WebSocketConfig, Message},
};
use utoipa::ToSchema;

#[derive(Clone, Serialize, ToSchema)]
pub struct Price {
    pub symbol: String,
    pub price: f64,
    pub size: f64,
    pub exchange_timestamp: String,
    pub received_at_ms: i64,
}

#[derive(Clone, Serialize, ToSchema)]
pub struct Snapshot {
    pub status: String,
    pub message: String,
    pub feed: String,
    pub symbols: Vec<String>,
    pub prices: BTreeMap<String, Price>,
    pub received_events: u64,
    pub reconnects: u64,
    pub journal: Option<String>,
    pub server_time_ms: i64,
    pub connection_attempts: u64,
    pub last_message_at_ms: Option<i64>,
    pub journal_bytes: u64,
    pub durable_seq: u64,
}
pub type Live = Arc<RwLock<Snapshot>>;

pub fn disabled() -> Live {
    Arc::new(RwLock::new(Snapshot {
        status: "disabled".into(),
        message: "Set QUANTCTL_LIVE_SYMBOLS, APCA_API_KEY_ID and APCA_API_SECRET_KEY in the server environment, then restart quantctl serve.".into(),
        feed: "iex".into(), symbols: vec![], prices: BTreeMap::new(),
        received_events: 0, reconnects: 0, journal: None, server_time_ms: 0,
        connection_attempts: 0, last_message_at_ms: None, journal_bytes: 0, durable_seq: 0,
    }))
}

struct Config {
    endpoint: String,
    feed: String,
    symbols: Vec<String>,
    key: String,
    secret: String,
}
impl Config {
    fn from_env() -> Result<Option<Self>, &'static str> {
        let Ok(symbols) = std::env::var("QUANTCTL_LIVE_SYMBOLS") else {
            return Ok(None);
        };
        Self::parse(
            symbols,
            std::env::var("QUANTCTL_ALPACA_FEED").unwrap_or_else(|_| "iex".into()),
            std::env::var("APCA_API_KEY_ID").unwrap_or_default(),
            std::env::var("APCA_API_SECRET_KEY").unwrap_or_default(),
        )
        .map(Some)
    }
    fn parse(
        symbols: String,
        feed: String,
        key: String,
        secret: String,
    ) -> Result<Self, &'static str> {
        if !matches!(feed.as_str(), "iex" | "sip" | "delayed_sip" | "test") {
            return Err("QUANTCTL_ALPACA_FEED must be iex, sip, delayed_sip or test");
        }
        let mut symbols: Vec<String> = symbols
            .split(',')
            .map(|s| s.trim().to_uppercase())
            .collect();
        symbols.sort();
        symbols.dedup();
        if symbols.is_empty()
            || symbols.len() > 30
            || symbols.iter().any(|s| {
                s.is_empty()
                    || s.len() > 15
                    || !s.bytes().all(|b| {
                        b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'.' || b == b'-'
                    })
            })
        {
            return Err("Configure 1–30 explicit stock symbols; wildcards are unsupported");
        }
        if feed == "test" && symbols != ["FAKEPACA"] {
            return Err("Alpaca test feed requires QUANTCTL_LIVE_SYMBOLS=FAKEPACA");
        }
        if key.trim().is_empty() || secret.trim().is_empty() {
            return Err("Missing APCA_API_KEY_ID or APCA_API_SECRET_KEY; configure credentials on the server and restart");
        }
        Ok(Self {
            endpoint: format!("wss://stream.data.alpaca.markets/v2/{feed}"),
            feed,
            symbols,
            key,
            secret,
        })
    }
}

pub struct Controller {
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}
impl Controller {
    pub async fn shutdown(mut self) {
        let _ = self.stop.send(true);
        if tokio::time::timeout(Duration::from_secs(45), &mut self.task)
            .await
            .is_err()
        {
            self.task.abort();
        }
    }
}
pub async fn start(root: &Path) -> (Live, Option<Controller>) {
    let live = disabled();
    let config = match Config::from_env() {
        Ok(Some(c)) => c,
        Ok(None) => return (live, None),
        Err(message) => {
            set_status(&live, "error", message).await;
            return (live, None);
        }
    };
    {
        let mut s = live.write().await;
        s.feed = config.feed.clone();
        s.symbols = config.symbols.clone();
    }
    let journal = match Journal::create(root, &config.feed, &config.symbols).await {
        Ok(journal) => journal,
        Err(Failure::Fatal(message)) => {
            set_status(&live, "error", &message).await;
            return (live, None);
        }
        Err(_) => unreachable!(),
    };
    live.write().await.journal = Some(journal.relative.clone());
    let (stop, receive) = watch::channel(false);
    let task = tokio::spawn(run(config, live.clone(), journal, receive));
    (live, Some(Controller { stop, task }))
}

async fn run(config: Config, state: Live, mut journal: Journal, mut stop: watch::Receiver<bool>) {
    let mut delay = 1;
    loop {
        if *stop.borrow() {
            break;
        }
        set_status(&state, "connecting", "Connecting to Alpaca").await;
        state.write().await.connection_attempts += 1;
        let started = tokio::time::Instant::now();
        let failure = session(&config, &state, &mut journal, &mut stop)
            .await
            .unwrap_err();
        if matches!(failure, Failure::Shutdown) {
            break;
        }
        if let Failure::Fatal(message) = failure {
            let _ = journal.finish("fatal").await;
            set_status(&state, "error", &message).await;
            return;
        }
        if started.elapsed() > Duration::from_secs(60) {
            delay = 1;
        }
        state.write().await.reconnects += 1;
        set_status(
            &state,
            "reconnecting",
            "Connection interrupted; retrying. Gap recorded; no automatic backfill.",
        )
        .await;
        if journal.append(&json!({"type":"gap"})).await.is_err() || journal.sync().await.is_err() {
            set_status(
                &state,
                "error",
                "Live journal unavailable or full; ingestion stopped",
            )
            .await;
            return;
        }
        update_storage(&state, &journal).await;
        // Jitter prevents synchronized reconnects across independent installations.
        let jitter = u64::from(uuid::Uuid::new_v4().as_bytes()[0]);
        tokio::select! {
            _ = stop.changed() => break,
            _ = tokio::time::sleep(Duration::from_millis(delay * 1000 + jitter)) => {}
        }
        delay = (delay * 2).min(30);
    }
    match journal.finish("shutdown").await {
        Ok(()) => set_status(&state, "stopped", "Capture closed and synced").await,
        Err(_) => {
            set_status(
                &state,
                "error",
                "Shutdown journal sync failed; capture is not clean",
            )
            .await
        }
    }
    update_storage(&state, &journal).await;
}
async fn update_storage(live: &Live, journal: &Journal) {
    let mut state = live.write().await;
    state.journal_bytes = journal.bytes;
    state.durable_seq = journal.durable_seq;
}

async fn set_status(live: &Live, status: &str, message: &str) {
    let mut s = live.write().await;
    s.status = status.into();
    s.message = message.into();
}

#[derive(Debug)]
enum Failure {
    Shutdown,
    Retry,
    Fatal(String),
}

async fn session(
    config: &Config,
    live: &Live,
    journal: &mut Journal,
    stop: &mut watch::Receiver<bool>,
) -> Result<(), Failure> {
    let limits = WebSocketConfig::default()
        .max_message_size(Some(1024 * 1024))
        .max_frame_size(Some(1024 * 1024));
    let (mut socket, _) = tokio::time::timeout(
        Duration::from_secs(15),
        connect_async_with_config(&config.endpoint, Some(limits), false),
    )
    .await
    .map_err(|_| Failure::Retry)?
    .map_err(classify_socket_error)?;
    send(
        &mut socket,
        Message::Text(
            json!({"action":"auth", "key":config.key, "secret":config.secret})
                .to_string()
                .into(),
        ),
    )
    .await
    .map_err(|_| Failure::Retry)?;
    set_status(live, "authenticating", "Authenticating with Alpaca").await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut subscribed = false;
    let mut authenticated = false;
    let mut checkpoint = tokio::time::interval(Duration::from_secs(1));
    let mut heartbeat = tokio::time::interval(Duration::from_secs(20));
    heartbeat.tick().await;
    let mut last_message = tokio::time::Instant::now();
    loop {
        if *stop.borrow() {
            return Err(Failure::Shutdown);
        }
        let message = tokio::select! {
            _ = stop.changed() => return Err(Failure::Shutdown),
            _ = checkpoint.tick() => { journal.sync().await?; update_storage(live, journal).await; continue; },
            _ = tokio::time::sleep_until(deadline), if !subscribed => return Err(Failure::Retry),
            _ = heartbeat.tick() => {
                if last_message.elapsed() > Duration::from_secs(60) { return Err(Failure::Retry); }
                send(&mut socket, Message::Ping(vec![].into())).await.map_err(|_| Failure::Retry)?;
                continue;
            }
            message = socket.next() => message.ok_or(Failure::Retry)?.map_err(classify_socket_error)?,
        };
        last_message = tokio::time::Instant::now();
        live.write().await.last_message_at_ms = Some(Utc::now().timestamp_millis());
        let text = match message {
            Message::Text(text) => text,
            Message::Ping(data) => {
                send(&mut socket, Message::Pong(data))
                    .await
                    .map_err(|_| Failure::Retry)?;
                continue;
            }
            Message::Close(_) => return Err(Failure::Retry),
            Message::Pong(_) => continue,
            _ => return Err(Failure::Fatal("Unsupported Alpaca frame format".into())),
        };
        if text.len() > 1024 * 1024 {
            return Err(Failure::Fatal(
                "Alpaca batch exceeds 1 MiB ingestion limit".into(),
            ));
        }
        let messages: Vec<Value> = serde_json::from_str(&text)
            .map_err(|_| Failure::Fatal("Malformed Alpaca event batch".into()))?;
        for event in messages {
            match event["T"].as_str().unwrap_or("") {
                "success" if event["msg"] == "authenticated" => {
                    if authenticated {
                        return Err(Failure::Fatal("Duplicate authentication response".into()));
                    }
                    authenticated = true;
                    send(&mut socket, Message::Text(json!({"action":"subscribe", "trades":config.symbols, "quotes":config.symbols, "bars":config.symbols, "updatedBars":config.symbols}).to_string().into())).await.map_err(|_| Failure::Retry)?;
                }
                "subscription" => {
                    let confirmed = authenticated
                        && ["trades", "quotes", "bars", "updatedBars"]
                            .iter()
                            .all(|channel| {
                                event[*channel].as_array().is_some_and(|symbols| {
                                    config.symbols.iter().all(|s| symbols.contains(&json!(s)))
                                })
                            });
                    if !confirmed {
                        return Err(Failure::Fatal(
                            "Alpaca did not confirm every requested channel after authentication"
                                .into(),
                        ));
                    }
                    subscribed = true;
                    set_status(live, "connected", "Subscribed; waiting for market events").await;
                }
                "error" => {
                    let code = event["code"].as_u64().unwrap_or(0);
                    if matches!(code, 406 | 407 | 500) {
                        return Err(Failure::Retry);
                    }
                    // Never forward arbitrary upstream text or credentials into logs/UI.
                    return Err(Failure::Fatal(format!("Alpaca rejected the stream (code {code}); check credentials, feed entitlement and symbol limits")));
                }
                "t" | "q" | "b" | "u" | "c" | "x" if subscribed => {
                    let Some(symbol) = event["S"]
                        .as_str()
                        .filter(|s| config.symbols.iter().any(|v| v == s))
                    else {
                        continue;
                    };
                    let timestamp = event["t"]
                        .as_str()
                        .and_then(|t| DateTime::parse_from_rfc3339(t).ok());
                    let Some(timestamp) = timestamp else {
                        return Err(Failure::Fatal(
                            "Market event has an invalid timestamp".into(),
                        ));
                    };
                    let now = Utc::now().timestamp_millis();
                    quant_data::capture::validate_event(&event, now).map_err(|_| {
                        Failure::Fatal("Invalid market event; ingestion stopped".into())
                    })?;
                    let price = if event["T"] == "t" {
                        Some(parse_trade(&event, now)?)
                    } else {
                        None
                    };
                    journal
                        .append(&json!({"type":"market","feed":config.feed,"received_at_ms":now,"event":event}))
                        .await?;
                    let mut s = live.write().await;
                    s.received_events += 1;
                    if let Some(price) = price {
                        let newer = s.prices.get(symbol).is_none_or(|old| {
                            DateTime::parse_from_rfc3339(&old.exchange_timestamp)
                                .is_ok_and(|old| timestamp >= old)
                        });
                        if newer {
                            s.prices.insert(symbol.into(), price);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn classify_socket_error(error: tokio_tungstenite::tungstenite::Error) -> Failure {
    use tokio_tungstenite::tungstenite::Error;
    match error {
        Error::Http(response) if matches!(response.status().as_u16(), 400 | 401 | 403 | 404) => {
            Failure::Fatal(format!(
                "Alpaca HTTP {}: check credentials and feed configuration",
                response.status().as_u16()
            ))
        }
        Error::Capacity(_) | Error::Protocol(_) | Error::Utf8(_) => {
            Failure::Fatal("Invalid or oversized provider WebSocket frame".into())
        }
        _ => Failure::Retry,
    }
}
async fn send<S>(
    socket: &mut tokio_tungstenite::WebSocketStream<S>,
    message: Message,
) -> Result<(), tokio_tungstenite::tungstenite::Error>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    tokio::time::timeout(Duration::from_secs(10), socket.send(message))
        .await
        .map_err(|_| {
            tokio_tungstenite::tungstenite::Error::Io(std::io::Error::from(
                std::io::ErrorKind::TimedOut,
            ))
        })?
}

fn parse_trade(event: &Value, received_at_ms: i64) -> Result<Price, Failure> {
    let price = event["p"].as_f64().filter(|p| p.is_finite() && *p > 0.0);
    let size = event["s"].as_f64().filter(|s| s.is_finite() && *s >= 0.0);
    match (price, size, event["S"].as_str(), event["t"].as_str()) {
        (Some(price), Some(size), Some(symbol), Some(timestamp))
            if DateTime::parse_from_rfc3339(timestamp).is_ok() =>
        {
            Ok(Price {
                symbol: symbol.into(),
                price,
                size,
                exchange_timestamp: timestamp.into(),
                received_at_ms,
            })
        }
        _ => Err(Failure::Fatal(
            "Invalid trade price, size or timestamp; ingestion stopped".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[test]
    fn configuration_and_error_classification_are_explicit() {
        for (symbols, feed, key) in [
            ("", "iex", "key"),
            ("*", "iex", "key"),
            ("../AAPL", "iex", "key"),
            ("AAPL", "unknown", "key"),
            ("AAPL", "test", "key"),
            ("AAPL", "iex", ""),
        ] {
            assert!(
                Config::parse(symbols.into(), feed.into(), key.into(), "secret".into()).is_err()
            );
        }
        let cfg = Config::parse(
            "aapl,MSFT,AAPL".into(),
            "iex".into(),
            "key".into(),
            "secret".into(),
        )
        .unwrap();
        assert_eq!(cfg.symbols, ["AAPL", "MSFT"]);
        assert_eq!(cfg.endpoint, "wss://stream.data.alpaca.markets/v2/iex");
        let response = tokio_tungstenite::tungstenite::http::Response::builder()
            .status(401)
            .body(Some(b"secret".to_vec()))
            .unwrap();
        let Failure::Fatal(message) = classify_socket_error(
            tokio_tungstenite::tungstenite::Error::Http(Box::new(response)),
        ) else {
            panic!("HTTP auth error must stop")
        };
        assert!(!message.contains("secret"));
        assert!(matches!(
            classify_socket_error(tokio_tungstenite::tungstenite::Error::ConnectionClosed),
            Failure::Retry
        ));
    }

    #[tokio::test]
    async fn reconnect_resubscribes_and_shutdown_seals_the_capture() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("ws://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            for index in 0..2 {
                let (stream, _) = listener.accept().await.unwrap();
                let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
                let auth: Value =
                    serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                assert_eq!(auth["action"], "auth");
                socket
                    .send(Message::Text(
                        json!([{"T":"success","msg":"authenticated"}])
                            .to_string()
                            .into(),
                    ))
                    .await
                    .unwrap();
                let sub: Value =
                    serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                assert_eq!(sub["trades"], json!(["AAPL"]));
                socket.send(Message::Text(json!([{"T":"subscription","trades":["AAPL"],"quotes":["AAPL"],"bars":["AAPL"],"updatedBars":["AAPL"]},
                    {"T":"t","S":"AAPL","p":100+index,"s":1,"t":Utc::now().to_rfc3339()}]).to_string().into())).await.unwrap();
                if index == 0 {
                    socket.close(None).await.unwrap();
                } else {
                    while let Some(Ok(_)) = socket.next().await {}
                }
            }
        });
        let temp = tempfile::tempdir().unwrap();
        let journal = Journal::create(temp.path(), "iex", &["AAPL".into()])
            .await
            .unwrap();
        let path = temp.path().join(&journal.relative);
        let state = disabled();
        let (stop, receive) = watch::channel(false);
        let task = tokio::spawn(run(
            Config {
                endpoint,
                feed: "iex".into(),
                symbols: vec!["AAPL".into()],
                key: "key".into(),
                secret: "secret".into(),
            },
            state.clone(),
            journal,
            receive,
        ));
        tokio::time::timeout(Duration::from_secs(8), async {
            while state.read().await.received_events < 2 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        stop.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap();
        server.await.unwrap();
        let audit = quant_data::capture::verify(&path).unwrap();
        assert!(audit.clean_shutdown);
        assert_eq!(audit.gaps, 1);
        let state = state.read().await;
        assert_eq!(state.status, "stopped");
        assert_eq!(state.connection_attempts, 2);
        assert_eq!(state.durable_seq, audit.records);
        assert_eq!(state.prices["AAPL"].price, 101.0);
    }

    #[test]
    fn rejects_invalid_trades() {
        let good =
            json!({"T":"t","S":"AAPL","p":123.5,"s":10,"t":"2026-09-28T14:00:00.123456789Z"});
        assert_eq!(parse_trade(&good, 42).unwrap().received_at_ms, 42);
        for (field, value) in [
            ("p", json!(-1)),
            ("s", json!(-2)),
            ("t", json!("bad")),
            ("p", Value::Null),
        ] {
            let mut bad = good.clone();
            bad[field] = value;
            assert!(parse_trade(&bad, 42).is_err());
        }
    }

    async fn exchange(responses: Vec<Value>) -> (Live, String, Failure) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("ws://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            let auth: Value =
                serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
            assert_eq!(
                auth,
                json!({"action":"auth","key":"test-key","secret":"test-secret"})
            );
            ws.send(Message::Text(
                json!([{"T":"success","msg":"authenticated"}])
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
            let subscribe: Value =
                serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
            assert_eq!(subscribe["action"], "subscribe");
            for channel in ["trades", "quotes", "bars", "updatedBars"] {
                assert_eq!(subscribe[channel], json!(["AAPL"]));
            }
            for response in responses {
                ws.send(Message::Text(response.to_string().into()))
                    .await
                    .unwrap();
            }
            let _ = ws.close(None).await;
        });
        let temp = tempfile::tempdir().unwrap();
        let mut journal = Journal::create(temp.path(), "iex", &["AAPL".into()])
            .await
            .unwrap();
        let path = temp.path().join(&journal.relative);
        let live = disabled();
        let config = Config {
            endpoint,
            feed: "iex".into(),
            symbols: vec!["AAPL".into()],
            key: "test-key".into(),
            secret: "test-secret".into(),
        };
        let (_sender, mut stop) = watch::channel(false);
        let failure = tokio::time::timeout(
            Duration::from_secs(5),
            session(&config, &live, &mut journal, &mut stop),
        )
        .await
        .unwrap()
        .unwrap_err();
        journal.sync().await.unwrap();
        server.await.unwrap();
        let contents = tokio::fs::read_to_string(path.join("000001.ndjson"))
            .await
            .unwrap();
        let record = contents
            .lines()
            .filter(|line| serde_json::from_str::<Value>(line).unwrap()["type"] == "market")
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!record.contains("test-secret"));
        (live, record, failure)
    }

    #[tokio::test]
    async fn websocket_authenticates_subscribes_persists_and_preserves_event_order() {
        let (live, records, failure) = exchange(vec![
            json!([{"T":"subscription","trades":["AAPL"],"quotes":["AAPL"],"bars":["AAPL"],"updatedBars":["AAPL"]}]),
            json!([
                {"T":"t","S":"AAPL","p":102,"s":2,"t":"2026-09-28T14:00:02Z"},
                {"T":"t","S":"AAPL","p":101,"s":1,"t":"2026-09-28T14:00:01Z"},
                {"T":"q","S":"AAPL","bp":101,"ap":103,"bs":1,"as":1,"t":"2026-09-28T14:00:02Z"},
                {"T":"u","S":"AAPL","o":102,"h":102,"l":102,"c":102,"v":2,"t":"2026-09-28T14:00:00Z"},
                {"T":"c","S":"AAPL","op":102,"cp":101,"os":1,"cs":1,"t":"2026-09-28T14:00:03Z"},
                {"T":"t","S":"MSFT","p":999,"s":1,"t":"2026-09-28T14:00:03Z"}
            ]),
        ])
        .await;
        assert!(matches!(failure, Failure::Retry));
        let state = live.read().await;
        assert_eq!(state.status, "connected");
        assert_eq!(state.prices["AAPL"].price, 102.0);
        assert_eq!(state.received_events, 5);
        let records: Vec<Value> = records
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        assert_eq!(records.len(), 5);
        assert_eq!(records[1]["event"]["p"], 101);
        assert!(records[0]["received_at_ms"].as_i64().unwrap() > 0);
    }

    #[tokio::test]
    async fn provider_rejection_is_terminal_and_redacts_upstream_text() {
        let (_, records, failure) =
            exchange(vec![json!([{"T":"error","code":409,"msg":"test-secret"}])]).await;
        assert!(records.is_empty());
        let Failure::Fatal(message) = failure else {
            panic!("must stop on entitlement failure")
        };
        assert!(message.contains("409"));
        assert!(!message.contains("test-secret"));
    }

    #[tokio::test]
    async fn incomplete_subscription_fails_instead_of_claiming_live() {
        let (state, _, failure) = exchange(vec![json!([{"T":"subscription","trades":[]}])]).await;
        assert!(matches!(failure, Failure::Fatal(_)));
        assert_ne!(state.read().await.status, "connected");
    }

    #[tokio::test]
    async fn invalid_trade_is_not_recorded_or_displayed() {
        let (state, records, failure) =
            exchange(vec![json!([{"T":"subscription","trades":["AAPL"],"quotes":["AAPL"],"bars":["AAPL"],"updatedBars":["AAPL"]},
            {"T":"t","S":"AAPL","p":-1,"s":2,"t":"2026-09-28T14:00:00Z"}])])
            .await;
        assert!(matches!(failure, Failure::Fatal(_)));
        assert!(records.is_empty());
        assert!(state.read().await.prices.is_empty());
    }
}
