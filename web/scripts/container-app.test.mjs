import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { test } from 'node:test';
import { chromium } from '@playwright/test';
import { tableFromIPC } from 'apache-arrow';
import { startContainerApp } from '../api/container.mjs';

const repo = resolve('..');
const binary = process.env.QUANTCTL_BIN ?? resolve(repo, 'rust/target/debug/quantctl' + (process.platform === 'win32' ? '.exe' : ''));
const bundle = JSON.parse(readFileSync(resolve(repo, 'deploy/artifacts.tar.json')));

test('single-container website preserves real inference, private backend and research navigation', { timeout: 90_000 }, async () => {
  const root = mkdtempSync(resolve(tmpdir(), 'lstm-container-app-'));
  execFileSync(process.env.PYTHON ?? 'python', [resolve(repo, 'scripts/deployment_bundle.py'),
    'extract', '--bundle', resolve(repo, 'deploy/artifacts.tar.gz'), '--output', root], { stdio: 'pipe' });
  let app, browser;
  try {
    app = await startContainerApp({ binary, root, assets: resolve('dist'), port: 0, host: '127.0.0.1' });
    assert.equal((await fetch(app.backend + '/api/models')).status, 401);
    assert.equal((await fetch(app.url + '/api/health')).status, 200);
    assert.equal((await fetch(app.url + '/api/runs', { method: 'POST' })).status, 405);
    assert.equal((await fetch(app.url + '/api/unknown')).status, 404);
    assert.equal((await fetch(app.url + '/assets/missing.js')).status, 404);
    assert.equal((await fetch(app.url + '/%2e%2e%2fpackage.json')).status, 404);
    assert.equal((await fetch(app.url + '/api/models?upstream=https://example.com')).status, 200);
    const response = await fetch(app.url + '/api/forecast?' + new URLSearchParams({ model: bundle.model, dataset: bundle.dataset, symbol: bundle.symbol }));
    assert.equal(response.status, 200, await response.clone().text());
    const forecast = await response.json();
    assert.equal(forecast.source, 'synthetic');
    assert.ok(forecast.implied_close > 0);
    assert.equal(forecast.dataset_sha256.length, 64);
    const runs = await (await fetch(app.url + '/api/runs')).json();
    const backtest = runs.find(run => run.kind === 'backtest');
    const simulation = runs.find(run => run.kind === 'simulation');
    const equity = await fetch(app.url + '/api/runs/' + backtest.run_id + '/equity.arrow');
    assert.ok(tableFromIPC(new Uint8Array(await equity.arrayBuffer())).numRows > 0);
    assert.equal((await (await fetch(app.url + '/api/market/status')).json()).configured, false);
    browser = await chromium.launch();
    const errors = [];
    for (const [label, viewport] of [['desktop', { width: 1440, height: 1000 }], ['mobile', { width: 390, height: 844 }]]) {
      const page = await browser.newPage({ viewport });
      page.on('pageerror', error => errors.push(error.message));
      await page.goto(app.url + '/forecast');
      await page.getByLabel('Forecast model').selectOption(bundle.model);
      await page.getByLabel('Forecast dataset').selectOption(bundle.dataset + '/' + bundle.symbol);
      await page.getByRole('button', { name: 'Generate forecast' }).click();
      const result = page.getByRole('region', { name: 'Forecast result' });
      await result.waitFor();
      assert.match(await result.innerText(), /Synthetic or fixture data/);
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
      await page.reload();
      await page.getByRole('heading', { name: 'Make a forecast', exact: true }).waitFor();
      await page.goto(app.url + '/risk');
      await page.getByLabel('Include in-sample and unverified runs').check();
      await page.getByLabel('Active run').selectOption(simulation.run_id);
      await page.getByRole('heading', { name: 'Outcome distributions', exact: true }).waitFor();
      assert.equal(new URL(page.url()).pathname, '/risk');
      mkdirSync(resolve('test-results/container-app'), { recursive: true });
      await page.screenshot({ path: resolve(`test-results/container-app/${label}-simulation.png`), fullPage: true });
      await page.goto(app.url + '/live');
      await page.waitForFunction(() => document.querySelector('[role="status"]')?.textContent?.includes('disabled'));
      await page.close();
    }
    assert.deepEqual(errors, []);
    await app.stop();
    await assert.rejects(fetch(app.url + '/api/health'));
    await assert.rejects(fetch(app.backend + '/api/health'));
    console.log(JSON.stringify({ status: 'passed', actualInference: true, viewports: 2, backendRequiresToken: true }));
  } finally { await browser?.close(); await app?.stop(); rmSync(root, { recursive: true, force: true, maxRetries: 5 }); }
});

test('container startup failure is actionable', async () => {
  await assert.rejects(startContainerApp({ binary: 'missing-quantctl-container-test', root: repo, assets: resolve('dist'), port: 0 }), /Could not start inference engine/);
});
