export const products = ['BTC-USD', 'ETH-USD', 'SOL-USD'] as const;
export const marketRest = 'https://api.exchange.coinbase.com';
export const marketSocket = 'wss://ws-feed.exchange.coinbase.com';
export type Instrument = { id: string; base: string; quote: string; name: string };
export function parseCatalog(value: unknown): Instrument[] {
  if (!Array.isArray(value)) throw new Error('Invalid market catalog');
  return value.filter(p => p && typeof p.id === 'string' && /^[A-Z0-9]+-[A-Z0-9]+$/.test(p.id) && p.status === 'online' && !p.trading_disabled && typeof p.base_currency === 'string' && typeof p.quote_currency === 'string').map(p => ({ id: p.id, base: p.base_currency, quote: p.quote_currency, name: typeof p.display_name === 'string' ? p.display_name : p.id })).sort((a, b) => a.id.localeCompare(b.id));
}
export async function marketCatalog(signal: AbortSignal): Promise<Instrument[]> {
  const response = await fetch(`${marketRest}/products`, { signal, credentials: 'omit' });
  if (!response.ok) throw new Error('Instrument catalog unavailable');
  return parseCatalog(await response.json());
}
export function numeric(value: unknown): number {
  if ((typeof value !== 'string' && typeof value !== 'number') || value === '') throw new Error('Invalid market value');
  const n = Number(value);
  if (!Number.isFinite(n) || n < 0) throw new Error('Invalid market value');
  return n;
}
export function candles(value: unknown): number[][] {
  if (!Array.isArray(value)) throw new Error('Invalid candles');
  const rows = value.map(row => {
    if (!Array.isArray(row) || row.length !== 6) throw new Error('Invalid candle');
    const r = row.map(numeric);
    if (!r[0] || !r[1] || r[1] > Math.min(r[3], r[4]) || r[2] < Math.max(r[3], r[4])) throw new Error('Invalid OHLC');
    return r;
  });
  return [...new Map(rows.map(r => [r[0], r])).values()].sort((a, b) => a[0] - b[0]).slice(-300);
}
export function levels(value: unknown, bids: boolean): number[][] {
  if (!Array.isArray(value)) throw new Error('Invalid book');
  return value.map(row => {
    if (!Array.isArray(row) || row.length < 2) throw new Error('Invalid level');
    const r = [numeric(row[0]), numeric(row[1])];
    if (!r[0]) throw new Error('Invalid price');
    return r;
  }).filter(r => r[1] > 0).sort((a, b) => bids ? b[0] - a[0] : a[0] - b[0]).slice(0, 10);
}
export async function marketGet(product: string, resource: string, signal: AbortSignal): Promise<unknown> {
  if (!/^[A-Z0-9]{1,20}-[A-Z0-9]{1,20}$/.test(product)) throw new Error('Unsupported product');
  const response = await fetch(`${marketRest}/products/${product}/${resource}`, { signal, credentials: 'omit' });
  if (!response.ok) throw new Error(`Market feed unavailable (${response.status})`);
  return response.json();
}
