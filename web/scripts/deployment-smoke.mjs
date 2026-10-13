// Uses the shipped bundle, real Rust inference, real HTTP bridge and production SPA.
// No route mocks or fabricated forecast responses.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { spawn, execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, existsSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, extname, sep } from 'node:path';
import { once } from 'node:events';
import { randomBytes } from 'node:crypto';
import { chromium } from '@playwright/test';
import { tableFromIPC } from 'apache-arrow';
import { createBridge } from '../api/bridge.js';

const repo = resolve('..');
const workspace = mkdtempSync(resolve(tmpdir(), 'lstm-deploy-'));
const bundle = JSON.parse(readFileSync(resolve(repo, 'deploy/artifacts.tar.json')));
execFileSync(process.env.PYTHON ?? 'python', [resolve(repo, 'scripts/deployment_bundle.py'),
  'extract', '--bundle', resolve(repo, 'deploy/artifacts.tar.gz'), '--output', workspace], { stdio: 'inherit' });
const token = randomBytes(32).toString('hex');
const binary = process.env.QUANTCTL_BIN ?? resolve(repo, 'rust/target/debug/quantctl' + (process.platform === 'win32' ? '.exe' : ''));
const env = { ...process.env, QUANTCTL_BACKEND_TOKEN: token };
delete env.QUANTCTL_LIVE_SYMBOLS;
delete env.APCA_API_KEY_ID;
delete env.APCA_API_SECRET_KEY;
delete env.TWELVE_DATA_API_KEY;
const child = spawn(binary, ['serve', '--remote', '--workers', '1', '--port', '0', '--root', workspace], { env, stdio: ['ignore', 'pipe', 'pipe'] });
let server, browser;
const failures = [];
try {
  const backend = await new Promise((resolveUrl, reject) => {
    let output = '';
    const timer = setTimeout(() => reject(new Error('Backend startup timeout: ' + output)), 30_000);
    child.on('error', reject);
    child.on('exit', code => { clearTimeout(timer); reject(new Error('Backend exited: ' + code + output)); });
    child.stderr.on('data', data => { output += data; });
    child.stdout.on('data', data => {
      output += data;
      const match = output.match(/Research inspector: http:\/\/0\.0\.0\.0:(\d+)/);
      if (match) { clearTimeout(timer); resolveUrl('http://127.0.0.1:' + match[1]); }
    });
  });
  assert.equal((await fetch(backend + '/api/health')).status, 200);
  assert.equal((await fetch(backend + '/api/models')).status, 401);
  let streamRequests = 0;
  const bridge = createBridge({ env: { QUANTCTL_BACKEND_URL: backend, QUANTCTL_BACKEND_TOKEN: token }, timeoutMs: 10_000 });
  const assets = resolve('dist');
  const mime = { '.html':'text/html', '.js':'text/javascript', '.css':'text/css', '.svg':'image/svg+xml' };
  server = createServer((req, res) => {
    if (req.url.startsWith('/api/')) {
      if (req.url.startsWith('/api/live/events')) streamRequests++;
      // Exercise the rewrite contract used in vercel.json.
      const url = new URL(req.url, 'http://local');
      url.searchParams.append('__path', url.pathname.slice(5));
      req.url = '/api/bridge?' + url.searchParams;
      return bridge(req, res);
    }
    const url = new URL(req.url, 'http://local');
    let file = resolve(assets, '.' + url.pathname);
    if (!file.startsWith(assets + sep) || !existsSync(file) || !statSync(file).isFile()) file = resolve(assets, 'index.html');
    res.writeHead(200, { 'content-type': mime[extname(file)] ?? 'application/octet-stream' });
    res.end(readFileSync(file));
  });
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const base = 'http://127.0.0.1:' + server.address().port;
  const runs = await (await fetch(base + '/api/runs')).json();
  assert.ok(runs.length > 0, 'Published research runs exist');
  const backtest = runs.find(run => run.kind === 'backtest');
  const arrow = await fetch(base + '/api/runs/' + backtest.run_id + '/equity.arrow?max_points=100');
  assert.equal(arrow.status, 200);
  assert.ok(tableFromIPC(new Uint8Array(await arrow.arrayBuffer())).numRows > 0);
  const forecastUrl = base + '/api/forecast?' + new URLSearchParams({model:bundle.model,dataset:bundle.dataset,symbol:bundle.symbol});
  const forecastResponse = await fetch(forecastUrl);
  assert.equal(forecastResponse.status, 200, await forecastResponse.clone().text());
  const forecast = await forecastResponse.json();
  assert.ok(Number.isFinite(forecast.implied_close) && forecast.implied_close > 0);
  assert.equal(forecast.source, bundle.source);
  assert.equal(forecast.dataset_sha256.length, 64);
  browser = await chromium.launch();
  for (const viewport of [{width:1440,height:1000}, {width:390,height:844}]) {
    const page = await browser.newPage({viewport});
    page.on('pageerror', error => failures.push(error.message));
    await page.goto(base + '/forecast');
    await page.getByLabel('Forecast model').selectOption(bundle.model);
    await page.getByLabel('Forecast dataset').selectOption(bundle.dataset + '/' + bundle.symbol);
    await page.getByRole('button', {name:'Generate forecast'}).click();
    await page.getByRole('region', {name:'Forecast result'}).waitFor();
    assert.match(await page.getByRole('region', {name:'Forecast result'}).innerText(), /Synthetic or fixture data/);
    await page.reload(); // Deep-link SPA routing works.
    await page.getByRole('heading', {name:'Make a forecast',exact:true}).waitFor();
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    await page.goto(base + '/');
    await page.getByRole('heading', {name:'A run, with its evidence'}).waitFor();
    await page.close();
  }
  const live = await browser.newPage();
  await live.goto(base + '/live');
  // The stream ends at our bounded proxy duration; EventSource must reconnect.
  await live.waitForFunction(() => document.querySelector('[role="status"]')?.textContent?.includes('disabled'));
  const initial = streamRequests;
  const deadline = Date.now() + 20_000;
  while (streamRequests <= initial && Date.now() < deadline) await new Promise(r => setTimeout(r, 250));
  assert.ok(streamRequests > initial, 'EventSource reconnects after the bridge closes its bounded stream');
  assert.deepEqual(failures, []);
  console.log(JSON.stringify({result:'passed',viewports:2,realForecast:forecast.implied_close,source:forecast.source,runCount:runs.length,sseReconnect:true,workspace}));
} finally {
  await browser?.close();
  if (server) { server.closeAllConnections(); server.close(); }
  if (child.pid && child.exitCode === null && child.signalCode === null) {
    const stopped = once(child, 'exit');
    child.kill('SIGTERM');
    const forceStop = setTimeout(() => child.kill('SIGKILL'), 5000);
    try { await stopped; } finally { clearTimeout(forceStop); }
  }
}
