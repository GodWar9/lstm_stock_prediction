export const products = ['BTC-USD', 'ETH-USD', 'SOL-USD'] as const;
export const marketRest = 'https://api.exchange.coinbase.com';
export const marketSocket = 'wss://ws-feed.exchange.coinbase.com';
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
  if (!(products as readonly string[]).includes(product)) throw new Error('Unsupported product');
  const response = await fetch(`${marketRest}/products/${product}/${resource}`, { signal, credentials: 'omit' });
  if (!response.ok) throw new Error(`Market feed unavailable (${response.status})`);
  return response.json();
}
