import { useEffect, useMemo, useState } from 'react';
import { candles, levels, marketGet, marketSocket, numeric, products, marketCatalog, type Instrument } from '../lib/market';
import { MarketChart } from './MarketChart';
import './terminal.css';
import { ResearchPanel } from './ResearchPanel';
import { SecuritiesExplorer, GlobalMarketPanels, type Security } from './GlobalMarkets';

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
const intervals = [[60, '1m'], [300, '5m'], [900, '15m'], [3600, '1h'], [86400, '1d']] as const;
function MarketPanels({ product, now }: { product: string; now: number }) {
  const [interval, setInterval] = useState(60);
  const currency = product.split('-')[1];
  const history = useSnapshot(product, `candles?granularity=${interval}`, 60_000, candles);
  const book = useSnapshot(product, 'book?level=2', 5000, parseBook);
  const trades = useSnapshot(product, 'trades?limit=30', 5000, parseTrades);
  const stamp = (s: { at: number; error?: string }, limit: number) => s.error ?? (!s.at ? 'Connecting…' : `${now - s.at > limit ? 'Stale · ' : ''}Updated ${new Date(s.at).toLocaleTimeString()}`);
  const bid = book.data?.bids[0]?.[0], ask = book.data?.asks[0]?.[0];
  const total = (side: 'bids' | 'asks') => book.data?.[side].reduce((sum, r) => sum + r[1], 0) ?? 0;
  return <div className="terminal-grid"><section className="terminal-chart terminal-panel"><div className="chart-toolbar"><h3>{product} <span>Price & volume</span></h3><div className="intervals" aria-label="Chart interval">{intervals.map(([value, label]) => <button key={value} aria-pressed={value === interval} onClick={() => setInterval(value)}>{label}</button>)}</div></div>
    <MarketChart rows={history.data ?? []} label={`${product} ${intervals.find(i => i[0] === interval)?.[1]}`} currency={currency}/><p className="panel-caption">{stamp(history, 90_000)} · Latest candle may be incomplete · Gaps are not filled</p></section>
    <section className="terminal-panel terminal-book"><div className="section-heading"><h3>Order book</h3><span>10 levels · 5s</span></div><div className="spread">Spread {currency === 'USD' ? '$' : ''}{fmt(bid !== undefined && ask !== undefined ? ask - bid : undefined)} <span>{currency}</span></div><div className="book-sides">{(['bids', 'asks'] as const).map(side => <div key={side}><table><thead><tr><th>{side === 'bids' ? 'Bid' : 'Ask'} ({currency})</th><th>Size</th></tr></thead><tbody>{book.data?.[side].map(([price, size]) => <tr key={price} className={side} style={{ backgroundImage: `linear-gradient(to left, ${side === 'bids' ? '#50c9a214' : '#ed7b8714'} ${Math.min(100, size / Math.max(total(side), .00000001) * 100)}%, transparent 0)` }}><td>{fmt(price, price < 1 ? 6 : 2)}</td><td>{fmt(size, 5)}</td></tr>)}</tbody></table></div>)}</div><p className="panel-caption">{stamp(book, 15_000)}</p></section>
    <section className="terminal-panel terminal-tape"><div className="section-heading"><h3>Recent trades</h3><span>Last 30 · 5s snapshots</span></div><div className="tape-scroll"><table><thead><tr><th>Time (UTC)</th><th>Price ({currency})</th><th>Quantity</th><th>Maker side</th></tr></thead><tbody>{trades.data?.map(t => <tr key={t.id}><td>{t.time.slice(11, 23)}</td><td className={t.side === 'buy' ? 'bids' : 'asks'}>{fmt(t.price, t.price < 1 ? 6 : 2)}</td><td>{fmt(t.size, 8)}</td><td>{t.side}</td></tr>)}</tbody></table></div><p className="panel-caption">{stamp(trades, 15_000)}</p></section></div>;
}
export function Terminal() {
  const [product, select] = useState<string>('BTC-USD');
  const [catalog, setCatalog] = useState<Instrument[]>(products.map(id => ({ id, base: id.split('-')[0], quote: 'USD', name: id })));
  const [catalogError, setCatalogError] = useState('');
  const [search, setSearch] = useState('');
  const [asset, setAsset] = useState('Crypto');
  const [security, selectSecurity] = useState<Security | null>(null);
  const [quoteCurrency, setQuoteCurrency] = useState('USD');
  const [coverage, showCoverage] = useState(false);
  const subscribed = useMemo(() => [...new Set([...products, product])], [product]);
  useEffect(() => {
    const controller = new AbortController();
    marketCatalog(AbortSignal.any([controller.signal, AbortSignal.timeout(10000)])).then(setCatalog).catch(() => { if (!controller.signal.aborted) setCatalogError('Catalog unavailable. Showing core pairs.'); });
    return () => controller.abort();
  }, []);
  const [quotes, setQuotes] = useState<Record<string, Quote>>({});
  const [status, setStatus] = useState('Connecting');
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    let stopped = false, socket: WebSocket, retry: ReturnType<typeof setTimeout>, attempts = 0, lastMessage = Date.now();
    const connect = () => {
      setStatus('Connecting'); lastMessage = Date.now();
      socket = new WebSocket(marketSocket);
      socket.onopen = () => socket.send(JSON.stringify({ type: 'subscribe', product_ids: subscribed, channels: ['ticker', 'heartbeat'] }));
      socket.onmessage = event => {
        if (stopped) return;
        try {
          const m = JSON.parse(event.data);
          if (m.type === 'error') { socket.close(); return; }
          if (!subscribed.includes(m.product_id)) return;
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
  }, [subscribed]);
  const q = quotes[product];
  const fresh = q && now - q.time < 30_000 && now - q.received < 15_000 && status === 'Connected';
  const matches = catalog.filter(p => (quoteCurrency === 'All' || p.quote === quoteCurrency) && `${p.id} ${p.name}`.toLowerCase().includes(search.toLowerCase()));
  const listed = [...matches].sort((a,b) => Number(subscribed.includes(b.id)) - Number(subscribed.includes(a.id))).slice(0, 60);
  const change = q?.open ? (q.price / q.open - 1) * 100 : undefined;
  return <div className="terminal"><div className="terminal-heading"><div><span className="terminal-eyebrow">MARKET INTELLIGENCE</span><h2>Market terminal<span className="beta-tag">{asset === 'Crypto' ? 'LIVE CRYPTO' : asset === 'Forex' ? 'DAILY REFERENCE' : 'SECURITIES CATALOGUE'}</span></h2><p>A clearer view of the market.</p></div><button className="coverage-button" onClick={() => showCoverage(!coverage)} aria-expanded={coverage}>Data coverage <span>↗</span></button></div>
    {coverage && <section className="coverage-panel"><h3>Your data connections</h3><div><p><b>Crypto · Connected</b><span>All active Coinbase spot pairs. Search by symbol and quote currency below.</span></p><p><b>Global stocks & forex</b><span>Forex uses free Frankfurter daily reference history. Stock listings are searchable; stock prices require a provider with public-display rights. The free Twelve Data account is not enabled for public prices.</span></p></div><a href="https://twelvedata.com/pricing-business" target="_blank" rel="noreferrer">Stock data licensing ↗</a><p>Exchange coverage, delays and depth depend on the plan. No single feed represents every venue.</p></section>}
    <div className="terminal-layout"><section className="market-explorer terminal-panel"><div className="section-heading"><h3>Markets</h3><span>{asset === 'Crypto' ? `${catalog.length} pairs` : 'Global catalogue'}</span></div><div className="asset-tabs" aria-label="Asset class">{['Crypto','Stocks','Forex'].map(name => <button key={name} aria-pressed={asset === name} onClick={() => { setAsset(name); selectSecurity(null); }}>{name}</button>)}</div>
    {asset === 'Crypto' ? <><label className="market-search"><span>⌕</span><input aria-label="Search markets" placeholder="Search symbol…" value={search} onChange={e => setSearch(e.target.value)}/></label><div className="quote-filter"><span>Quote currency</span><select aria-label="Quote currency" value={quoteCurrency} onChange={e => setQuoteCurrency(e.target.value)}>{['All', ...new Set(catalog.map(p => p.quote))].sort().map(c => <option key={c}>{c}</option>)}</select></div>{catalogError && <p className="panel-caption">{catalogError}</p>}<div className="market-list" aria-label="Market watchlist">{listed.map(p => { const v = quotes[p.id], delta = v?.open ? (v.price/v.open-1)*100 : undefined; return <button key={p.id} aria-pressed={p.id === product} onClick={() => select(p.id)}><span className="coin-icon">{p.base.slice(0,1)}</span><span className="market-name"><b>{p.id}</b><small>{p.name}</small></span><span className="market-price"><b>{v ? fmt(v.price, v.price < 1 ? 6 : 2) : 'View'}</b><small className={delta !== undefined && delta >= 0 ? 'bids' : 'asks'}>{delta === undefined ? 'Spot market' : `${fmt(delta)}%${now-v.time > 30000 || status !== 'Connected' ? ' · stale' : ''}`}</small></span></button>; })}</div><p className="panel-caption">{matches.length ? `Showing ${listed.length} of ${matches.length} pairs. Search to narrow results.` : 'No matching markets.'}</p></> : <SecuritiesExplorer key={asset} asset={asset} selected={security} onSelect={selectSecurity}/>}</section>
    <div className="market-workspace">{asset !== 'Crypto' ? <GlobalMarketPanels key={asset} security={security} asset={asset}/> : <><section className="instrument-overview"><div className="instrument-title"><span className="coin-icon large">{product.split('-')[0].slice(0,1)}</span><div><h3>{product}<span>Coinbase Exchange</span></h3><span className="instrument-kind">Spot crypto · {product.split('-')[1]}</span></div><span className={`feed-status ${fresh ? 'online' : ''}`} role="status">● {fresh ? 'Live' : q ? 'Stale quote' : status}</span></div><div className="instrument-summary"><div className="last-price">{fmt(q?.price, q && q.price < 1 ? 6 : 2)}<span className={change !== undefined && change >= 0 ? 'bids' : 'asks'}>{change !== undefined && change >= 0 ? '+' : ''}{fmt(change)}% <small>24h</small></span></div><div className="terminal-metrics">{[['24h high', fmt(q?.high)], ['24h low', fmt(q?.low)], ['24h volume · base', fmt(q?.volume)], ['Last tick · UTC', q ? new Date(q.time).toISOString().slice(11, 19) : '—']].map(([label, value]) => <div key={label}><span>{label}</span><strong>{value}</strong></div>)}</div></div></section><MarketPanels key={product} product={product} now={now}/></>}</div><ResearchPanel/></div>
    <footer className="terminal-footer"><span><i/> Coinbase Exchange · Public spot feed</span><span>₹0 data setup · Live crypto · Daily forex · Stock catalogue</span><details><summary>About this data</summary><p>Crypto prices come from Coinbase; forex reference rates come from Frankfurter. Daily reference rates are not streaming quotes. Data is acquired while this page is open and is not continuously archived. Market depth and trades are snapshots; they are not a complete execution feed. The forecast panel runs backend LSTM inference on its explicitly selected research dataset.</p></details></footer></div>;
}
