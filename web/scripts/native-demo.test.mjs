import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { test } from 'node:test';
import { chromium } from '@playwright/test';
import { startNativeDemo } from './native-demo.mjs';

test('native app serves real bundled inference on desktop and mobile and cleans up', { timeout: 90_000 }, async () => {
  const demo = await startNativeDemo({ build: false });
  let browser;
  try {
    assert.equal((await fetch(demo.url + '/api/health')).status, 200);
    const bundle = JSON.parse(readFileSync(new URL('../../deploy/artifacts.tar.json', import.meta.url)));
    const response = await fetch(demo.url + '/api/forecast?' + new URLSearchParams({model:bundle.model,dataset:bundle.dataset,symbol:bundle.symbol}));
    assert.equal(response.status, 200, await response.clone().text());
    const forecast = await response.json();
    assert.equal(forecast.source, 'synthetic');
    assert.ok(forecast.implied_close > 0);
    assert.equal(forecast.dataset_sha256.length, 64);
    browser = await chromium.launch();
    for (const viewport of [{width:1440,height:1000}, {width:390,height:844}]) {
      const page = await browser.newPage({ viewport });
      const errors = [];
      page.on('pageerror', error => errors.push(error.message));
      await page.goto(demo.url + '/forecast');
      await page.getByLabel('Forecast model').selectOption(bundle.model);
      await page.getByLabel('Forecast dataset').selectOption(bundle.dataset + '/' + bundle.symbol);
      await page.getByRole('button', {name:'Generate forecast'}).click();
      await page.getByRole('region', {name:'Forecast result'}).waitFor();
      assert.match(await page.getByRole('region', {name:'Forecast result'}).innerText(), /Synthetic or fixture data/);
      await page.reload();
      await page.getByRole('heading', {name:'Make a forecast',exact:true}).waitFor();
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
      await page.goto(demo.url + '/live');
      await page.waitForFunction(() => document.querySelector('[role="status"]')?.textContent?.includes('disabled'));
      assert.deepEqual(errors, []);
      await page.close();
    }
  } finally { await browser?.close(); await demo.stop(); }
  assert.equal(existsSync(demo.workspace), false);
  await assert.rejects(fetch(demo.url + '/api/health'));
  await demo.stop(); // Cleanup is idempotent.
});

test('failed startup removes its temporary workspace', async () => {
  let workspace;
  await assert.rejects(startNativeDemo({ build:false, binary:'missing-quantctl-native-test', onWorkspace:path => { workspace = path; } }), /Could not start quantctl/);
  assert.ok(workspace);
  assert.equal(existsSync(workspace), false);
});

test('invalid ports are rejected before allocating resources', async () => {
  for (const port of [-1, 65536, NaN, 1.5])
    await assert.rejects(startNativeDemo({build:false,port}), /Port must/);
});
