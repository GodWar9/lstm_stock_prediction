import type { components } from '../api/generated';
export type LiveSnapshot = components['schemas']['Snapshot'];
const record = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value);
const counter = (value: unknown): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
const timestamp = (value: unknown): value is number => counter(value) && value > 0;

export function parseLiveSnapshot(value: unknown): LiveSnapshot {
  if (!record(value) || !['disabled', 'connecting', 'authenticating', 'connected', 'reconnecting', 'error', 'stopped'].includes(String(value.status))
    || typeof value.message !== 'string' || !['iex', 'sip', 'delayed_sip', 'test'].includes(String(value.feed))
    || !Array.isArray(value.symbols) || value.symbols.length > 30 || !value.symbols.every(s => typeof s === 'string' && /^[A-Z0-9.-]{1,15}$/.test(s))
    || new Set(value.symbols).size !== value.symbols.length || !record(value.prices)
    || !timestamp(value.server_time_ms) || !['received_events', 'reconnects', 'connection_attempts', 'journal_bytes', 'durable_seq'].every(k => counter(value[k]))
    || !(value.journal === null || typeof value.journal === 'string')
    || !(value.latest_bars === undefined || record(value.latest_bars))
    || !(value.live_forecasts === undefined || record(value.live_forecasts))) throw new Error('Invalid live snapshot');
  for (const [symbol, price] of Object.entries(value.prices)) {
    if (!value.symbols.includes(symbol) || !record(price) || price.symbol !== symbol
      || typeof price.price !== 'number' || !Number.isFinite(price.price) || price.price <= 0
      || !counter(price.size) || !timestamp(price.received_at_ms)
      || typeof price.exchange_timestamp !== 'string' || !Number.isFinite(Date.parse(price.exchange_timestamp))) throw new Error('Invalid live price');
  }
  for (const [symbol, bar] of Object.entries(value.latest_bars ?? {})) {
    if (!value.symbols.includes(symbol) || !record(bar)
      || !['open', 'high', 'low', 'close'].every(k => typeof bar[k] === 'number' && Number.isFinite(bar[k]) && (bar[k] as number) > 0)
      || !counter(bar.volume) || !timestamp(bar.close_time_ms) || !timestamp(bar.availability_ms)
      || bar.availability_ms < bar.close_time_ms || typeof bar.timestamp !== 'string' || Date.parse(bar.timestamp) !== bar.close_time_ms) throw new Error('Invalid live bar');
  }
  for (const [symbol, forecast] of Object.entries(value.live_forecasts ?? {})) {
    if (!value.symbols.includes(symbol) || !record(forecast) || forecast.symbol !== symbol
      || typeof forecast.model !== 'string' || typeof forecast.dataset !== 'string'
      || !['as_of_ms', 'available_at_ms', 'generated_at_ms'].every(k => timestamp(forecast[k]))
      || !['last_close', 'predicted_return', 'predicted_log_return', 'implied_close'].every(k => typeof forecast[k] === 'number' && Number.isFinite(forecast[k]))
      || typeof forecast.feed_latency !== 'string' || typeof forecast.status !== 'string') throw new Error('Invalid live forecast');
  }
  return value as unknown as LiveSnapshot;
}

export function freshTrade(price: LiveSnapshot['prices'][string], serverNow: number): boolean {
  const receiptAge = serverNow - price.received_at_ms;
  const exchangeAge = serverNow - Date.parse(price.exchange_timestamp);
  return receiptAge >= 0 && receiptAge <= 30_000 && exchangeAge >= 0 && exchangeAge <= 30_000;
}
