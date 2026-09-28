import { useEffect, useState } from 'react';
import type { components } from '../api/generated';
import { number } from '../lib/format';

type Snapshot = components['schemas']['Snapshot'];
export function Live() {
  const [snapshot, setSnapshot] = useState<Snapshot>();
  const [transport, setTransport] = useState('Connecting to local feed');
  const [lastMessage, setLastMessage] = useState(0);
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const events = new EventSource('/api/live/events');
    events.addEventListener('live', event => {
      try {
        const data = JSON.parse(event.data) as Snapshot;
        if (!Array.isArray(data.symbols) || !data.prices || typeof data.server_time_ms !== 'number') throw new Error('Invalid snapshot');
        setSnapshot(data); setLastMessage(Date.now()); setTransport('Connected to local feed');
      } catch { setTransport('Invalid feed response'); }
    });
    events.onerror = () => setTransport('Local connection lost; reconnecting');
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => { events.close(); clearInterval(timer); };
  }, []);
  const disconnected = transport !== 'Connected to local feed' || now - lastMessage > 5000;
  const serverNow = (snapshot?.server_time_ms ?? now) + Math.max(0, now - lastMessage);
  return <>
    <div className="page-title"><h2>Live market data</h2><p>Continuous Alpaca stock WebSocket ingestion, recorded by Rust. Browser prices update once per second.</p></div>
    <section className="panel">
      <h2>Feed connection</h2>
      <p role="status">{disconnected ? (transport === 'Connected to local feed' ? 'Local feed is stale; waiting for updates' : transport) : `${snapshot?.status}: ${snapshot?.message}`}</p>
      {snapshot && <>
        <p>Provider: Alpaca · Feed: <strong>{snapshot.feed}</strong> · Recorded events: {snapshot.received_events} · Reconnects: {snapshot.reconnects}</p>
        {snapshot.feed === 'iex' && <p>IEX covers one exchange, not the consolidated US market.</p>}
        {snapshot.feed === 'delayed_sip' && <p className="warning">This feed is delayed by 15 minutes.</p>}
        {snapshot.feed === 'test' && <p className="warning">Alpaca test data — not real market prices.</p>}
        {snapshot.status === 'disabled' && <p>Configure the server environment: <code>QUANTCTL_LIVE_SYMBOLS=AAPL,MSFT</code>, <code>APCA_API_KEY_ID</code>, <code>APCA_API_SECRET_KEY</code>. Optional: <code>QUANTCTL_ALPACA_FEED=iex</code>. Restart the server after configuring it. Never enter API keys in this browser.</p>}
        {snapshot.status === 'error' && <p role="alert">{snapshot.message}</p>}
        {snapshot.journal && <p>Session journal: <code>{snapshot.journal}</code></p>}
      </>}
    </section>
    <section className="panel"><h2>Latest observed trades</h2>
      <p>Freshness is based on each trade’s exchange timestamp and local receipt time. A quiet or closed market can produce stale prices even while connected.</p>
      <div className="table-scroll"><table aria-label="Live prices"><thead><tr><th>Symbol</th><th>Price</th><th>Size</th><th>Exchange time (UTC)</th><th>Freshness</th></tr></thead><tbody>
        {snapshot?.symbols.map(symbol => {
          const price = snapshot.prices[symbol];
          const age = price ? Math.max(serverNow - price.received_at_ms, serverNow - Date.parse(price.exchange_timestamp)) : Infinity;
          const fresh = !disconnected && snapshot.status === 'connected' && age >= 0 && age <= 30_000;
          return <tr key={symbol}><th scope="row">{symbol}</th><td>{price ? number(price.price, 4) : '—'}</td><td>{price ? number(price.size, 0) : '—'}</td><td>{price?.exchange_timestamp ?? 'Awaiting first trade'}</td><td>{!price ? 'Waiting' : fresh ? 'Fresh' : 'Stale'}</td></tr>;
        })}
      </tbody></table></div>
    </section>
    <p className="method-note">These are observed trades, not executable quotes or model signals. Quotes, minute bars, revisions and trade corrections are retained in the journal; the price table does not reconstruct corrected trade history. Reconnect gaps are recorded and are not backfilled. Existing daily models, backtests and Monte Carlo reports remain separate: intraday replay needs bar aggregation, gap validation and a model trained for that interval.</p>
  </>;
}
