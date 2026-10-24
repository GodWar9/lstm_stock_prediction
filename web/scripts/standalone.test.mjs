import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { test } from 'node:test';
import { chromium } from '@playwright/test';
import { tableFromIPC } from 'apache-arrow';

const source = process.env.LSTM_INSPECTOR_BIN ?? resolve('../rust/target/debug/lstm-inspector' + (process.platform === 'win32' ? '.exe' : ''));
const bundle = JSON.parse(readFileSync(new URL('../../deploy/artifacts.tar.json', import.meta.url)));
function isolated() {
  const root = mkdtempSync(resolve(tmpdir(), 'lstm-portable-acceptance-'));
  const cwd = resolve(root, 'empty working directory');
  const temp = resolve(root, 'runtime temp');
  const binDir = resolve(root, 'app with spaces');
  for (const path of [cwd, temp, binDir]) mkdirSync(path);
  const binary = resolve(binDir, 'lstm-inspector' + (process.platform === 'win32' ? '.exe' : ''));
  copyFileSync(source, binary);
  const env = { ...process.env, PATH: '', TMP: temp, TEMP: temp, TMPDIR: temp,
    QUANTCTL_LIVE_SYMBOLS: 'THIS_MUST_BE_IGNORED', APCA_API_KEY_ID: 'test-not-a-secret',
    APCA_API_SECRET_KEY: 'test-not-a-secret', TWELVE_DATA_API_KEY: 'test-not-a-secret',
    QUANTCTL_ALLOW_NETWORK: '1', QUANTCTL_BACKEND_TOKEN: 'test-not-a-secret' };
  return { root, cwd, temp, binary, env };
}

test('single executable verifies bundled artifacts without PATH tools and cleans up', () => {
  const app = isolated();
  try {
    const result = JSON.parse(execFileSync(app.binary, ['--check'], { cwd: app.cwd, env: app.env, encoding: 'utf8', timeout: 30_000 }));
    assert.equal(result.status, 'verified');
    assert.equal(result.source, 'synthetic');
    assert.equal(result.model, bundle.model);
    assert.equal(result.bundle_sha256, bundle.bundle_sha256);
    assert.equal(result.files, Object.keys(bundle.files).length);
    assert.deepEqual(readdirSync(app.temp), []);
    assert.deepEqual(readdirSync(app.cwd), []);
  } finally { rmSync(app.root, { recursive: true, force: true }); }
});

test('occupied port fails clearly and removes the private workspace', async () => {
  const app = isolated();
  const server = createServer();
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  try {
    const child = spawn(app.binary, ['--no-browser', '--port', String(server.address().port)], { cwd: app.cwd, env: app.env, stdio: 'pipe' });
    let output = '';
    child.stdout.on('data', data => { output += data; });
    child.stderr.on('data', data => { output += data; });
    const [code] = await once(child, 'close');
    assert.notEqual(code, 0);
    assert.match(output, /address|socket|permission/i);
    assert.deepEqual(readdirSync(app.temp), []);
  } finally { server.close(); rmSync(app.root, { recursive: true, force: true }); }
});

test('copied standalone app serves real forecasts, Arrow and all research pages', { timeout: 180_000 }, async () => {
  const app = isolated();
  const child = spawn(app.binary, ['--no-browser'], { cwd: app.cwd, env: app.env, stdio: 'pipe' });
  const closed = once(child, 'close');
  let browser;
  try {
    const base = await new Promise((resolveUrl, reject) => {
      let output = '';
      const timer = setTimeout(() => reject(new Error('Portable app startup timed out')), 30_000);
      child.once('error', error => { clearTimeout(timer); reject(error); });
      child.once('exit', code => { clearTimeout(timer); reject(new Error(`Portable app exited: ${code}: ${output}`)); });
      child.stderr.on('data', data => { output = (output + data).slice(-8192); });
      child.stdout.on('data', data => {
        output = (output + data).slice(-8192);
        const match = output.match(/Research inspector: (http:\/\/127\.0\.0\.1:\d+)/);
        if (match) { clearTimeout(timer); resolveUrl(match[1]); }
      });
    });
    assert.equal((await fetch(base + '/api/health')).status, 200);
    const response = await fetch(base + '/api/forecast?' + new URLSearchParams({ model: bundle.model, dataset: bundle.dataset, symbol: bundle.symbol }));
    assert.equal(response.status, 200, await response.clone().text());
    const forecast = await response.json();
    assert.equal(forecast.source, 'synthetic');
    assert.ok(Number.isFinite(forecast.implied_close) && forecast.implied_close > 0);
    assert.match(forecast.dataset_sha256, /^[a-f0-9]{64}$/);
    const market = await (await fetch(base + '/api/market/status')).json();
    assert.equal(market.configured, false, 'Inherited provider credentials are ignored');
    const runs = await (await fetch(base + '/api/runs')).json();
    const backtest = runs.find(run => run.kind === 'backtest');
    assert.ok(backtest);
    const simulation = runs.find(run => run.kind === 'simulation');
    assert.ok(simulation);
    const bands = await fetch(base + '/api/runs/' + simulation.run_id + '/simulation/bands.arrow');
    assert.equal(bands.status, 200);
    assert.ok(tableFromIPC(new Uint8Array(await bands.arrayBuffer())).numRows > 0);
    const arrow = await fetch(base + '/api/runs/' + backtest.run_id + '/equity.arrow');
    assert.equal(arrow.status, 200);
    assert.ok(tableFromIPC(new Uint8Array(await arrow.arrayBuffer())).numRows > 0);
    browser = await chromium.launch();
    const errors = [];
    const remoteRequests = [];
    for (const [label, viewport] of [['desktop', { width: 1440, height: 1000 }], ['mobile', { width: 390, height: 844 }]]) {
      const page = await browser.newPage({ viewport });
      page.on('pageerror', error => errors.push(error.message));
      page.on('request', request => { if (/^https?:/.test(request.url()) && !request.url().startsWith(base + '/')) remoteRequests.push(request.url()); });
      await page.goto(base + '/forecast');
      await page.getByLabel('Forecast model').selectOption(bundle.model);
      await page.getByLabel('Forecast dataset').selectOption(bundle.dataset + '/' + bundle.symbol);
      await page.getByRole('button', { name: 'Generate forecast' }).click();
      const result = page.getByRole('region', { name: 'Forecast result' });
      await result.waitFor();
      assert.match(await result.innerText(), /Synthetic or fixture data/);
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
      mkdirSync(resolve('test-results/standalone'), { recursive: true });
      await page.screenshot({ path: resolve(`test-results/standalone/${label}-forecast.png`), fullPage: true });
      await page.reload();
      await page.getByRole('heading', { name: 'Make a forecast', exact: true }).waitFor();
      for (const [path, heading] of [['/', 'A run, with its evidence'], ['/backtest', 'Backtest inspection'],
        ['/validation', 'Data and validation'], ['/models', 'Model artifacts'], ['/signals', 'Signal inspection'],
        ['/risk', 'Risk and simulation'], ['/jobs', 'Run activity']]) {
        await page.goto(base + path);
        await page.getByRole('heading', { name: heading, exact: true }).waitFor();
        assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true, `${label} ${path} overflows`);
        assert.equal(await page.getByRole('alert').count(), 0, `${label} ${path} has an error`);
      }
      await page.goto(base + '/risk');
      await page.getByLabel('Include in-sample and unverified runs').check();
      await page.getByLabel('Active run').selectOption(simulation.run_id);
      try {
        await page.getByRole('heading', { name: 'Outcome distributions', exact: true }).waitFor({ timeout: 5000 });
      } catch (error) {
        throw new Error(`Simulation selection failed at ${page.url()}: ${await page.locator('main').innerText()}`, { cause: error });
      }
      assert.equal(await page.getByRole('alert').count(), 0);
      await page.goto(base + '/live');
      await page.waitForFunction(() => document.querySelector('[role="status"]')?.textContent?.includes('disabled'));
      await page.close();
    }
    assert.deepEqual(errors, []);
    assert.deepEqual(remoteRequests, []);
    assert.deepEqual(readdirSync(app.cwd), []);
    console.log(JSON.stringify({ status: 'passed', source: forecast.source, viewports: 2, routes: 9, impliedClose: forecast.implied_close, runCount: runs.length, toolsOnPath: false }));
  } finally {
    await browser?.close();
    if (child.exitCode === null && child.signalCode === null) child.kill('SIGTERM');
    await closed;
    // Windows forcibly terminates Node child processes; the test owns this exact
    // temporary directory. Interactive Ctrl+C uses the server's graceful path.
    assert.ok(app.root.startsWith(resolve(tmpdir(), 'lstm-portable-acceptance-')));
    rmSync(app.root, { recursive: true, force: true, maxRetries: 5 });
  }
});
