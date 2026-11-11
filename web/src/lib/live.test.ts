import { describe, expect, test } from 'vitest';
import { freshTrade, parseLiveSnapshot } from './live';

const now = Date.parse('2026-09-28T14:00:00Z');
const price = { symbol: 'AAPL', price: 123, size: 10, exchange_timestamp: new Date(now).toISOString(), received_at_ms: now };
const snapshot = { status: 'connected', message: 'Subscribed', feed: 'iex', symbols: ['AAPL'], prices: { AAPL: price }, received_events: 1, reconnects: 0, connection_attempts: 1, journal_bytes: 100, durable_seq: 2, server_time_ms: now, last_message_at_ms: now, journal: 'datasets/live/test' };
describe('live snapshot contract', () => {
  test('accepts valid snapshots and empty disabled state', () => {
    expect(parseLiveSnapshot(snapshot).prices.AAPL.price).toBe(123);
    expect(parseLiveSnapshot({ ...snapshot, status: 'disabled', symbols: [], prices: {}, journal: null, last_message_at_ms: null }).status).toBe('disabled');
  });
  test.each([null, [], { ...snapshot, prices: { AAPL: {} } }, { ...snapshot, status: 'pretend-live' }, { ...snapshot, durable_seq: -1 }, { ...snapshot, symbols: ['AAPL', 'AAPL'] }, { ...snapshot, prices: { AAPL: { ...price, price: Infinity } } }])('rejects malformed snapshots: %j', data => {
    expect(() => parseLiveSnapshot(data)).toThrow();
  });
  test('old or future timestamps cannot be fresh', () => {
    expect(freshTrade(price, now)).toBe(true);
    expect(freshTrade(price, now + 30_001)).toBe(false);
    expect(freshTrade({ ...price, exchange_timestamp: new Date(now + 1000).toISOString() }, now)).toBe(false);
    expect(freshTrade({ ...price, received_at_ms: now + 1 }, now)).toBe(false);
  });
});

test('validates completed bars and forecasts before rendering', () => {
  const bar = { timestamp: new Date(now).toISOString(), close_time_ms: now, availability_ms: now + 500, open: 100, high: 101, low: 99, close: 100, volume: 10 };
  expect(parseLiveSnapshot({ ...snapshot, latest_bars: { AAPL: bar } }).latest_bars?.AAPL.close).toBe(100);
  for (const latest_bars of [{ AAPL: {} }, { AAPL: { ...bar, close: Infinity } }, { MSFT: bar }, { AAPL: { ...bar, availability_ms: now - 1 } }]) {
    expect(() => parseLiveSnapshot({ ...snapshot, latest_bars })).toThrow();
  }
  expect(() => parseLiveSnapshot({ ...snapshot, live_forecasts: { AAPL: { predicted_return: null } } })).toThrow();
});
