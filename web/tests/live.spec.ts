import { test, expect, type Page } from '@playwright/test';

// Browser transport harness: the Rust tests separately exercise a real local
// WebSocket peer, including authentication, subscriptions, journals and errors.
async function controlledFeed(page: Page) {
  await page.addInitScript(() => {
    const Native = window.EventSource;
    class MockSource extends EventTarget {
      onerror: (() => void) | null = null;
      constructor() {
        super();
        window.addEventListener('test-feed', this.receive);
        window.addEventListener('test-feed-error', this.fail);
      }
      receive = (event: Event) => this.dispatchEvent(new MessageEvent('live', { data: JSON.stringify((event as CustomEvent).detail) }));
      fail = () => this.onerror?.();
      close() {
        window.removeEventListener('test-feed', this.receive);
        window.removeEventListener('test-feed-error', this.fail);
      }
    }
    window.EventSource = function (url: string | URL) {
      return String(url) === '/api/live/events' ? new MockSource() : new Native(url);
    } as unknown as typeof EventSource;
  });
  await page.route('**/api/runs', route => route.fulfill({ status: 500, json: { message: 'Historical artifacts unavailable' } }));
  await page.goto('/live');
  await expect(page.getByRole('heading', { name: 'Live market data', exact: true })).toBeVisible();
}

async function emit(page: Page, overrides: Record<string, unknown> = {}) {
  await page.evaluate(overrides => {
    const now = Date.now();
    window.dispatchEvent(new CustomEvent('test-feed', { detail: {
      status: 'connected', message: 'Subscribed', feed: 'iex', symbols: ['AAPL'],
      prices: { AAPL: { symbol: 'AAPL', price: 123.45, size: 10, exchange_timestamp: new Date(now).toISOString(), received_at_ms: now } },
      received_events: 1, reconnects: 0, connection_attempts: 1, journal_bytes: 100, durable_seq: 2, last_message_at_ms: now, journal: 'datasets/live/test.ndjson', server_time_ms: now,
      ...overrides,
    } }));
  }, overrides);
}

test('unconfigured Rust feed explains setup without requiring saved runs', async ({ page, request }) => {
  const snapshot = await (await request.get('/api/live')).json();
  expect(snapshot.status).toBe('disabled');
  expect(snapshot.prices).toEqual({});
  await page.route('**/api/runs', route => route.fulfill({ json: [] }));
  await page.goto('/live');
  await expect(page.getByRole('status')).toContainText('disabled');
  await expect(page.getByText('Never enter API keys in this browser.', { exact: false })).toBeVisible();
  await expect(page.getByLabel('Active run')).toHaveCount(0);
  await expect(page.getByRole('alert')).toHaveCount(0);
});

test('incoming prices update and old exchange timestamps remain stale', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await controlledFeed(page);
  await emit(page);
  const row = page.getByRole('row').filter({ has: page.getByRole('rowheader', { name: 'AAPL' }) });
  await expect(row).toContainText('123.4500');
  await expect(row).toContainText('Fresh');
  await emit(page, { prices: { AAPL: { symbol: 'AAPL', price: 124.5, size: 2, exchange_timestamp: '2020-01-01T00:00:00Z', received_at_ms: Date.now() } }, received_events: 2 });
  await expect(row).toContainText('124.5000');
  await expect(row).toContainText('Stale');
  expect(errors).toEqual([]);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBeTruthy();
});

test('upstream reconnect and local disconnect never display fresh prices', async ({ page }) => {
  await controlledFeed(page);
  await emit(page);
  await expect(page.getByRole('cell', { name: 'Fresh', exact: true })).toBeVisible();
  await emit(page, { status: 'reconnecting', message: 'Retrying; gap recorded', reconnects: 1 });
  await expect(page.getByRole('status')).toContainText('reconnecting');
  await expect(page.getByRole('cell', { name: 'Stale', exact: true })).toBeVisible();
  await emit(page);
  await page.evaluate(() => window.dispatchEvent(new Event('test-feed-error')));
  await expect(page.getByRole('status')).toContainText('Local connection lost');
  await expect(page.getByRole('cell', { name: 'Stale', exact: true })).toBeVisible();
  await emit(page);
  await expect(page.getByRole('cell', { name: 'Fresh', exact: true })).toBeVisible();
});

test('stalled browser stream expires freshness even without an error event', async ({ page }) => {
  await page.clock.install();
  await controlledFeed(page);
  await emit(page);
  await expect(page.getByRole('cell', { name: 'Fresh', exact: true })).toBeVisible();
  await page.clock.fastForward(6000);
  await expect(page.getByRole('status')).toContainText('Local feed is stale');
  await expect(page.getByRole('cell', { name: 'Stale', exact: true })).toBeVisible();
});

test('missing trades, entitlement failures and test feed are explicit', async ({ page }) => {
  await controlledFeed(page);
  await emit(page, { prices: {} });
  await expect(page.getByRole('cell', { name: 'Waiting', exact: true })).toBeVisible();
  await emit(page, { status: 'error', message: 'Alpaca rejected the stream (code 409)' });
  await expect(page.getByRole('alert')).toContainText('409');
  await expect(page.getByRole('cell', { name: 'Stale', exact: true })).toBeVisible();
  await emit(page, { feed: 'test' });
  await expect(page.getByText('Alpaca test data — not real market prices.')).toBeVisible();
  await emit(page, { feed: 'delayed_sip' });
  await expect(page.getByText('This feed is delayed by 15 minutes.')).toBeVisible();
});
