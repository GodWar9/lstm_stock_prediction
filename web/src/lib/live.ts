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
    || !(value.last_message_at_ms === null || timestamp(value.last_message_at_ms))
    || !(value.journal === null || typeof value.journal === 'string')) throw new Error('Invalid live snapshot');
  for (const [symbol, price] of Object.entries(value.prices)) {
    if (!value.symbols.includes(symbol) || !record(price) || price.symbol !== symbol
      || typeof price.price !== 'number' || !Number.isFinite(price.price) || price.price <= 0
      || !counter(price.size) || !timestamp(price.received_at_ms)
      || typeof price.exchange_timestamp !== 'string' || !Number.isFinite(Date.parse(price.exchange_timestamp))) throw new Error('Invalid live price');
  }
  return value as unknown as LiveSnapshot;
}

export function freshTrade(price: LiveSnapshot['prices'][string], serverNow: number): boolean {
  const receiptAge = serverNow - price.received_at_ms;
  const exchangeAge = serverNow - Date.parse(price.exchange_timestamp);
  return receiptAge >= 0 && receiptAge <= 30_000 && exchangeAge >= 0 && exchangeAge <= 30_000;
}
