import { tableFromArrays, tableToIPC } from 'apache-arrow';
import { test, expect } from '@playwright/test';
const pages = [
  ['/', 'A run, with its evidence'], ['/backtest', 'Backtest inspection'],
  ['/validation', 'Data and validation'], ['/models', 'Model artifacts'],
  ['/signals', 'Signal inspection'], ['/risk', 'Risk and simulation'], ['/jobs', 'Run activity'],
];
test('all inspector routes work with external requests blocked', async ({ page, context }) => {
  const external: string[] = [];
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await context.route('**/*', route => {
    if (new URL(route.request().url()).origin !== 'http://127.0.0.1:8788') {
      external.push(route.request().url());
      return route.abort('internetdisconnected');
    }
    return route.continue();
  });
  for (const [path, title] of pages) {
    const response = await page.goto(path);
    expect(response?.headers()['content-security-policy']).toContain("connect-src 'self'");
    await expect(page.getByRole('heading', { name: title, exact: true })).toBeVisible();
    await expect(page.getByRole('alert')).toHaveCount(0);
  }
  expect(external).toEqual([]);
  expect(errors).toEqual([]);
});
for (const [path, title] of pages) {
  test(`${title} renders against Rust artifacts`, async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', e => errors.push(e.message));
    await page.goto(path);
    await expect(page.getByRole('heading', { name: title, exact: true })).toBeVisible();
    await expect(page.getByText('Evidence trail', { exact: true })).toBeVisible();
    await expect(page.getByRole('alert')).toHaveCount(0);
    if (path === '/' || path === '/backtest' || path === '/signals') await expect(page.locator('canvas').first()).toBeVisible();
    if (path === '/validation') await expect(page.getByText('Scaler fitting', { exact: true })).toBeVisible();
    expect(errors).toEqual([]);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBeTruthy();
  });
}
test('displays exact backend metrics and accessible chart data', async ({ page, request }) => {
  const runs = await (await request.get('/api/runs')).json();
  const run = runs.find((r: { split: string }) => r.split === 'test');
  await page.goto(`/?run=${run.run_id}`);
  await expect(page.getByText(new Intl.NumberFormat('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(run.metrics.sharpe), { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'View data table', exact: true }).click();
  await expect(page.getByRole('table', { name: 'Series observations' })).toBeVisible();
});
test('shows simulation after explicit unverified toggle', async ({ page, request }) => {
  const runs = await (await request.get('/api/runs')).json();
  const run = runs.find((r: { kind: string }) => r.kind === 'simulation');
  await page.goto(`/risk?run=${run.run_id}&inSample=true`);
  await expect(page.getByRole('heading', { name: 'Outcome distributions' })).toBeVisible();
  await expect(page.locator('canvas').first()).toBeVisible();
});
test('empty and API error states are actionable', async ({ page }) => {
  await page.route('**/api/runs', route => route.fulfill({ json: [] }));
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Your research starts with a recorded run' })).toBeVisible();
  await page.route('**/api/runs', route => route.fulfill({ status: 500, json: { code: 'test_error', message: 'Deliberate API error state test' } }));
  await page.reload();
  await expect(page.getByRole('alert')).toContainText('Deliberate API error state test');
});

test('captures chart timing and workspace appearance', async ({ page, request }, testInfo) => {
  const runs = await (await request.get('/api/runs')).json();
  const run = runs.find((r: { split: string }) => r.split === 'test');
  const response = await request.get('/api/runs/' + run.run_id + '/equity.arrow?max_points=2000');
  const bytes = (await response.body()).byteLength;
  const started = Date.now();
  await page.goto('/backtest?run=' + run.run_id);
  await expect(page.locator('canvas').first()).toBeVisible();
  const elapsed = Date.now() - started;
  await testInfo.attach('chart-performance', { body: JSON.stringify({ payload_bytes: bytes, navigation_to_first_chart_ms: elapsed, run_id: run.run_id }), contentType: 'application/json' });
  await page.screenshot({ path: testInfo.outputPath('backtest.png'), fullPage: true });
});

test('new runs expose benchmark, positions, signal outcomes and risk', async ({ page, request }, testInfo) => {
  const runs = await (await request.get('/api/runs')).json();
  const run = runs.find((r: { capabilities: string[] }) => r.capabilities.includes('risk'));
  expect(run).toBeTruthy();
  await page.goto(`/backtest?run=${run.run_id}`);
  await expect(page.getByRole('heading', { name: 'Benchmark buy-and-hold · account currency' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Position history · shares' })).toBeVisible();
  await expect(page.locator('canvas')).toHaveCount(4);
  await page.goto(`/signals?run=${run.run_id}`);
  await expect(page.getByRole('heading', { name: 'Signal outcomes · expected vs realized return' })).toBeVisible();
  await expect(page.locator('canvas')).toHaveCount(3);
  await page.goto(`/risk?run=${run.run_id}`);
  await expect(page.getByRole('heading', { name: 'Point-in-time risk evaluation' })).toBeVisible();
  await expect(page.getByText('MarketCrash_10Pct', { exact: true })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath('risk-evidence.png'), fullPage: true });
  await expect(page.getByRole('alert')).toHaveCount(0);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBeTruthy();
});


test('complete audit table navigates without skipping rows', async ({ page, request }) => {
  const runs = await (await request.get('/api/runs')).json();
  const run = runs.find((r: { capabilities: string[] }) => r.capabilities.includes('trades'));
  const offsets: number[] = [];
  await page.route('**/trades.arrow?*', async route => {
    const url = new URL(route.request().url());
    expect(url.searchParams.has('max_points')).toBeFalsy();
    const offset = Number(url.searchParams.get('offset'));
    const limit = Number(url.searchParams.get('limit'));
    offsets.push(offset);
    const count = Math.min(limit, 205 - offset);
    const rows = Array.from({ length: count }, (_, i) => offset + i);
    const bytes = tableToIPC(tableFromArrays({ timestamp_ms: Float64Array.from(rows), quantity: Float64Array.from(rows) }), 'file');
    await route.fulfill({ contentType: 'application/vnd.apache.arrow.file', body: Buffer.from(bytes), headers: {
      'x-total-count': '205', 'x-offset': String(offset), 'x-returned-count': String(count), 'x-sampled': 'false',
    } });
  });
  await page.goto(`/backtest?run=${run.run_id}`);
  await expect(page.getByText('Records 1-100 of 205', { exact: false })).toBeVisible();
  await page.getByRole('button', { name: 'Next page', exact: true }).click();
  await expect(page.getByText('Records 101-200 of 205', { exact: false })).toBeVisible();
  await page.getByRole('button', { name: 'Next page', exact: true }).click();
  await expect(page.getByText('Records 201-205 of 205', { exact: false })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Next page', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Previous page', exact: true }).click();
  await expect(page.getByText('Records 101-200 of 205', { exact: false })).toBeVisible();
  expect(offsets).toEqual([0, 100, 200]);
});
