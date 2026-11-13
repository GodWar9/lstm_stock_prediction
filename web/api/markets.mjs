// Public reference catalogs and credential-isolated market snapshots.
import { readFile } from 'node:fs/promises';
import { gunzipSync } from 'node:zlib';
const origin = 'https://api.twelvedata.com/';
const intervals = new Set(['1min', '5min', '15min', '1h', '1day']);
export function createMarkets({ env = process.env, fetchImpl = fetch, now = Date.now, catalogFile = new URL('../data/securities.json.gz', import.meta.url) } = {}) {
  const cache = new Map(), pending = new Map();
  let reference;
  async function bundledCatalog() {
    if (!catalogFile) return null;
    reference ??= readFile(catalogFile).then(bytes => JSON.parse(gunzipSync(bytes, { maxOutputLength: 96 * 1024 * 1024 }).toString('utf8'))).catch(() => null);
    return reference;
  }
  let minute = 0, minuteCount = 0, day = 0, dayCount = 0;
  const configured = () => Boolean(env.TWELVE_DATA_API_KEY && env.TWELVE_DATA_PUBLIC_DISPLAY === '1');
  async function load(endpoint, params, ttl, paid = false) {
    const id = endpoint + JSON.stringify(params);
    if (cache.has(id) && now() - cache.get(id).at < ttl) return cache.get(id).data;
    if (pending.has(id)) return pending.get(id);
    const request = (async () => {
      if (paid) {
        if (!configured()) throw Object.assign(new Error('Connect a server-side Twelve Data key and confirm public-display rights to enable prices and history.'), { status: 503 });
        const m = Math.floor(now() / 60000), d = Math.floor(now() / 86400000);
        if (m !== minute) { minute = m; minuteCount = 0; }
        if (d !== day) { day = d; dayCount = 0; }
        if (minuteCount >= 4 || dayCount >= 400) throw Object.assign(new Error('Market-data request budget reached. Cached data remains available; retry later.'), { status: 429 });
        minuteCount++; dayCount++;
      }
      const url = new URL(endpoint, origin);
      for (const [key, value] of Object.entries(params)) if (value) url.searchParams.set(key, value);
      const response = await fetchImpl(url, { headers: paid ? { Authorization: `apikey ${env.TWELVE_DATA_API_KEY}` } : {}, signal: AbortSignal.timeout(25000), redirect: 'error' });
      if (!response.ok) throw new Error('Market provider unavailable');
      let size = 0; const chunks = [];
      for await (const chunk of response.body) {
        size += chunk.length;
        if (size > 32 * 1024 * 1024) throw new Error('Provider response exceeds limit');
        chunks.push(chunk);
      }
      const data = JSON.parse(Buffer.concat(chunks).toString('utf8'));
      if (data.status === 'error') throw new Error('Provider rejected this instrument or plan entitlement');
      if (cache.size >= 128) cache.delete(cache.keys().next().value);
      cache.set(id, { data, at: now() }); return data;
    })();
    pending.set(id, request);
    try { return await request; } finally { pending.delete(id); }
  }
  return async function markets(req, res) {
    const send = (status, body) => { res.writeHead(status, { 'content-type': 'application/json', 'cache-control': 'no-store' }); res.end(JSON.stringify(body)); };
    if (req.method !== 'GET') return send(405, { message: 'Read-only endpoint' });
    try {
      const url = new URL(req.url, 'http://local');
      const action = url.pathname.split('/').pop();
      if (action === 'status') return send(200, { configured: configured(), provider: 'Twelve Data', priceMode: 'snapshot', displayConfirmed: env.TWELVE_DATA_PUBLIC_DISPLAY === '1', freeForex: { provider: 'Frankfurter', mode: 'daily-reference', keyRequired: false } });
      if (action === 'reference') {
        const symbol = url.searchParams.get('symbol') ?? '';
        if (!/^[A-Z]{3}\/[A-Z]{3}$/.test(symbol)) return send(400, { message: 'Choose a currency pair such as EUR/USD.' });
        const [base, quote] = symbol.split('/');
        if (base === quote) return send(400, { message: 'Choose two different currencies.' });
        const end = new Date(now()).toISOString().slice(0, 10);
        const start = new Date(now() - 365 * 86400000).toISOString().slice(0, 10);
        const raw = await load('https://api.frankfurter.dev/v2/rates', { base, quotes: quote, from: start, to: end }, 6 * 3600000);
        if (!Array.isArray(raw)) throw new Error('Invalid reference series');
        const seen = new Set();
        const points = raw.map(r => {
          const date = typeof r.date === 'string' ? r.date : '';
          const stamp = Date.parse(date + 'T00:00:00Z');
          if (!/^\d{4}-\d{2}-\d{2}$/.test(date) || !Number.isFinite(stamp) || new Date(stamp).toISOString().slice(0,10) !== date || date < start || date > end || r.base !== base || r.quote !== quote || typeof r.rate !== 'number' || !Number.isFinite(r.rate) || r.rate <= 0 || seen.has(date)) throw new Error('Invalid reference observation');
          seen.add(date);
          return { date, rate: r.rate };
        }).sort((a, b) => a.date.localeCompare(b.date));
        if (!points.length) return send(404, { message: 'No free daily reference history is available for this pair.' });
        const last = points.at(-1), previous = points.at(-2);
        const recent = points.slice(-30).map(p => p.rate);
        return send(200, { symbol, currency: quote, source: 'Frankfurter blended reference rates', sourceUrl: 'https://frankfurter.dev/', mode: 'daily-reference', asOf: last.date, fetchedAt: new Date(now()).toISOString(), stale: now() - Date.parse(last.date + 'T00:00:00Z') > 7 * 86400000, points, metrics: { last: last.rate, changePct: previous ? (last.rate / previous.rate - 1) * 100 : null, low30: Math.min(...recent), high30: Math.max(...recent), mean30: recent.reduce((a,b) => a+b, 0) / recent.length, observations: points.length, windowObservations: recent.length }, delay: 'Daily reference rates, not executable or streaming quotes. Missing dates are not filled.' });
      }
      if (action === 'catalog') {
        const asset = url.searchParams.get('asset');
        if (!['stocks', 'forex'].includes(asset)) return send(400, { message: 'Choose stocks or forex' });
        const bundled = await bundledCatalog();
        const raw = bundled?.catalogs?.[asset] ? { data: bundled.catalogs[asset] } : await load(asset === 'stocks' ? 'stocks' : 'forex_pairs', {}, 12 * 3600000);
        if (!Array.isArray(raw.data)) throw new Error('Invalid instrument catalog');
        const q = (url.searchParams.get('q') ?? '').trim().toLowerCase().slice(0, 100);
        const exchange = (url.searchParams.get('exchange') ?? '').slice(0, 80);
        const all = raw.data.filter(r => typeof r.symbol === 'string').map(r => ({ symbol: r.symbol, name: r.name ?? `${r.currency_base ?? ''} / ${r.currency_quote ?? ''}`, exchange: r.exchange ?? 'Forex', currency: r.currency ?? r.symbol.split('/')[1], country: r.country ?? '', type: r.type ?? 'Physical Currency' }));
        const filtered = all.filter(r => (!exchange || r.exchange === exchange) && `${r.symbol} ${r.name} ${r.exchange} ${r.country}`.toLowerCase().includes(q));
        const offset = Math.max(0, Math.min(1000000, Number(url.searchParams.get('offset')) || 0));
        return send(200, { source: 'Twelve Data reference catalog', asOf: bundled?.fetchedAt ?? null, total: filtered.length, catalogTotal: all.length, offset, items: filtered.slice(offset, offset + 50), exchanges: [...new Set(all.map(r => r.exchange))].sort(), pricesConfigured: configured() });
      }
      if (action === 'snapshot') {
        const expectedCurrency = url.searchParams.get('currency') ?? '';
        const symbol = url.searchParams.get('symbol') ?? '', exchange = url.searchParams.get('exchange') ?? '', interval = url.searchParams.get('interval') ?? '1day';
        if (!/^[\p{L}\p{N}./^:_ -]{1,50}$/u.test(symbol) || exchange.length > 80 || !intervals.has(interval)) return send(400, { message: 'Invalid instrument or interval' });
        const raw = await load('time_series', { symbol, exchange: exchange === 'Forex' ? '' : exchange, interval, outputsize: '300', timezone: 'UTC', order: 'asc' }, 60000, true);
        if (!Array.isArray(raw.values) || !raw.meta || String(raw.meta.symbol).toUpperCase() !== symbol.toUpperCase()) throw new Error('Invalid provider history');
        const bars = raw.values.map(r => [Date.parse(r.datetime.includes(' ') ? r.datetime.replace(' ', 'T') + 'Z' : r.datetime + 'T00:00:00Z') / 1000, Number(r.low), Number(r.high), Number(r.open), Number(r.close), Number(r.volume ?? 0)]);
        const currency = raw.meta.currency ?? (symbol.includes('/') ? symbol.split('/')[1] : '');
        if (expectedCurrency && currency !== expectedCurrency) throw new Error('Provider currency mismatch');
        if (!bars.length || bars.some(r => r.some(v => !Number.isFinite(v) || v < 0) || !r[1] || r[1] > Math.min(r[3], r[4]) || r[2] < Math.max(r[3], r[4]))) throw new Error('Invalid price history');
        return send(200, { source: 'Twelve Data', symbol: raw.meta.symbol, exchange: raw.meta.exchange ?? exchange, currency, interval, bars, asOf: bars[bars.length - 1][0] * 1000, delay: 'Provider entitlement dependent; not verified', volumeAvailable: raw.values.some(r => r.volume !== undefined) });
      }
      return send(404, { message: 'Unknown market endpoint' });
    } catch (error) { return send(error.status ?? 502, { message: error.status ? error.message : 'Market provider unavailable or this instrument is not included in the plan. Retry later.' }); }
  };
}
