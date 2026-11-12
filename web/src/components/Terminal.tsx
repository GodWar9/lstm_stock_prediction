import { useEffect, useMemo, useState } from 'react';
import { candles, levels, marketGet, marketSocket, numeric, products } from '../lib/market';
import { Chart } from './Chart';
import './terminal.css';

type Quote = { price: number; open: number; high: number; low: number; volume: number; time: number; received: number };
type Trade = { id: number; price: number; size: number; side: string; time: string };
const fmt = (v: number | undefined, digits = 2) => v === undefined ? '—' : v.toLocaleString('en-US', { maximumFractionDigits: digits, minimumFractionDigits: digits });
function useSnapshot<T>(product: string, resource: string, interval: number, parse: (v: unknown) => T) {
  const [state, set] = useState<{ data?: T; at: number; error?: string }>({ at: 0 });
  useEffect(() => {
    let active = true, timer: ReturnType<typeof setTimeout>;
    const controller = new AbortController();
    set({ at: 0 });
    const read = async () => {
      try {
        const data = parse(await marketGet(product, resource, AbortSignal.any([controller.signal, AbortSignal.timeout(10_000)])));
        if (active) set({ data, at: Date.now() });
      } catch { if (active) set(previous => ({ ...previous, error: 'Feed unavailable; retrying' })); }
      if (active) timer = setTimeout(read, interval);
    };
    void read();
    return () => { active = false; controller.abort(); clearTimeout(timer); };
  }, [product, resource, interval, parse]);
  return state;
}
const parseBook = (v: unknown) => {
  const b = v as { bids: unknown; asks: unknown };
  return { bids: levels(b.bids, true), asks: levels(b.asks, false) };
};
const parseTrades = (v: unknown): Trade[] => {
  if (!Array.isArray(v)) throw new Error('Invalid trades');
  return v.slice(0, 30).map(t => {
    if (!['buy', 'sell'].includes(t.side) || !Number.isFinite(Date.parse(t.time))) throw new Error('Invalid trade');
    return { id: numeric(t.trade_id), price: numeric(t.price), size: numeric(t.size), side: t.side, time: t.time };
  });
};
function MarketPanels({ product, now }: { product: string; now: number }) {
  const history = useSnapshot(product, 'candles?granularity=60', 60_000, candles);
  const book = useSnapshot(product, 'book?level=2', 5000, parseBook);
  const trades = useSnapshot(product, 'trades?limit=30', 5000, parseTrades);
  const stamp = (s: { at: number; error?: string }, limit: number) => s.error ?? (!s.at ? 'Connecting…' : `${now - s.at > limit ? 'Stale · ' : ''}Received ${new Date(s.at).toLocaleTimeString()}`);
  const series = useMemo(() => ({ timestamp_ms: history.data?.map(r => r[0] * 1000) ?? [], close: history.data?.map(r => r[4]) ?? [] }), [history.data]);
  const bid = book.data?.bids[0]?.[0], ask = book.data?.asks[0]?.[0];
  return <div className="terminal-grid"><div className="terminal-chart"><Chart title={`${product} · 1-minute close · USD`} data={series} x="timestamp_ms" ys={['close']} time intraday /><p className="feed-note">{stamp(history, 90_000)} · Refreshes every minute. Latest candle may be incomplete; missing intervals are not filled.</p></div>
    <section className="panel terminal-book"><h2>Order book</h2><p className="feed-note">Top 10 levels · 5s snapshots · {stamp(book, 15_000)}</p><strong>Spread ${fmt(bid !== undefined && ask !== undefined ? ask - bid : undefined)}</strong><div className="book-sides">{(['bids', 'asks'] as const).map(side => <div key={side}><h3>{side === 'bids' ? 'Bids' : 'Asks'}</h3><table><thead><tr><th>USD</th><th>{product.split('-')[0]}</th></tr></thead><tbody>{book.data?.[side].map(([price, size]) => <tr key={price} className={side}><td>{fmt(price)}</td><td>{fmt(size, 8)}</td></tr>)}</tbody></table></div>)}</div></section>
    <section className="panel terminal-tape"><h2>Trade tape</h2><p className="feed-note">Latest 30 · 5s snapshots · {stamp(trades, 15_000)}</p><div className="tape-scroll"><table><thead><tr><th>Time UTC</th><th>USD</th><th>Size</th><th>Maker side</th></tr></thead><tbody>{trades.data?.map(t => <tr key={t.id}><td>{t.time.slice(11, 23)}</td><td>{fmt(t.price)}</td><td>{fmt(t.size, 8)}</td><td className={t.side === 'buy' ? 'bids' : 'asks'}>{t.side}</td></tr>)}</tbody></table></div></section></div>;
}
export function Terminal() {
  const [product, select] = useState<string>('BTC-USD');
  const [quotes, setQuotes] = useState<Record<string, Quote>>({});
  const [status, setStatus] = useState('Connecting');
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    let stopped = false, socket: WebSocket, retry: ReturnType<typeof setTimeout>, attempts = 0, lastMessage = Date.now();
    const connect = () => {
      setStatus('Connecting'); lastMessage = Date.now();
      socket = new WebSocket(marketSocket);
      socket.onopen = () => socket.send(JSON.stringify({ type: 'subscribe', product_ids: products, channels: ['ticker', 'heartbeat'] }));
      socket.onmessage = event => {
        if (stopped) return;
        try {
          const m = JSON.parse(event.data);
          if (m.type === 'error') { socket.close(); return; }
          if (!products.includes(m.product_id)) return;
          lastMessage = Date.now(); attempts = 0; setStatus('Connected');
          if (m.type !== 'ticker') return;
          const time = Date.parse(m.time);
          if (!Number.isFinite(time)) return;
          const q = { price: numeric(m.price), open: numeric(m.open_24h), high: numeric(m.high_24h), low: numeric(m.low_24h), volume: numeric(m.volume_24h), time, received: Date.now() };
          setQuotes(previous => previous[m.product_id]?.time > time ? previous : { ...previous, [m.product_id]: q });
        } catch { /* Invalid messages cannot replace a valid quote. */ }
      };
      socket.onerror = () => socket.close();
      socket.onclose = () => { if (!stopped) { setStatus('Disconnected · reconnecting'); retry = setTimeout(connect, Math.min(30_000, 1000 * 2 ** attempts++)); } };
    };
    connect();
    const clock = setInterval(() => { setNow(Date.now()); if (Date.now() - lastMessage > 15_000 && socket.readyState < 2) socket.close(); }, 1000);
    return () => { stopped = true; clearInterval(clock); clearTimeout(retry); socket.close(); };
  }, []);
  const q = quotes[product];
  const fresh = q && now - q.time < 30_000 && now - q.received < 15_000 && status === 'Connected';
  return <div className="terminal"><div className="terminal-heading"><div><span className="terminal-eyebrow">COINBASE EXCHANGE / SPOT CRYPTO</span><h2>Market terminal</h2><p>Public market data · No account required · USD pairs</p></div><span className={`feed-status ${fresh ? 'online' : ''}`} role="status">● {fresh ? 'Live' : q ? 'Stale quote' : status}</span></div>
    <div className="watchlist" aria-label="Market watchlist">{products.map(p => { const v = quotes[p]; const change = v?.open ? (v.price / v.open - 1) * 100 : undefined; return <button key={p} aria-pressed={p === product} onClick={() => select(p)}><span>{p}</span><strong>${fmt(v?.price)}</strong><small className={change !== undefined && change >= 0 ? 'bids' : 'asks'}>{fmt(change)}% / 24h{v && now - v.time > 30_000 ? ' · stale' : ''}</small></button>; })}</div>
    <div className="terminal-metrics">{[['24h high', fmt(q?.high)], ['24h low', fmt(q?.low)], ['24h volume · base', fmt(q?.volume)], ['Exchange tick · UTC', q ? new Date(q.time).toISOString().slice(11, 23) : '—']].map(([label, value]) => <div key={label}><span>{label}</span><strong>{value}</strong></div>)}</div>
    <MarketPanels key={product} product={product} now={now} /><p className="feed-note">Source: Coinbase Exchange. Quotes stream over WebSocket; book, trades and candles use public REST snapshots. This screen acquires data while open; it does not archive a continuous feed or generate model predictions.</p></div>;
}
