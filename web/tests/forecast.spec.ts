import { test, expect } from '@playwright/test';

test('forecast page works without recorded runs and explains empty datasets', async ({ page }) => {
  await page.route('**/api/runs', route => route.fulfill({ status: 500, json: { message: 'No history' } }));
  await page.goto('/forecast');
  await expect(page.getByRole('heading', { name: 'Make a forecast', exact: true })).toBeVisible();
  await expect(page.getByText('No recorded datasets are available.', { exact: false })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Generate forecast' })).toBeDisabled();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBeTruthy();
});

test('rejects incompatible intervals before making a prediction request', async ({ page }) => {
  await page.route('**/api/datasets', route => route.fulfill({ json: [{ dataset: 'minute_capture', symbol: 'AAPL', interval: '1m', source: 'alpaca:iex:first_publication', bars: 1000 }] }));
  await page.goto('/forecast');
  await page.getByLabel('Forecast model').selectOption('inspector_demo');
  await page.getByLabel('Forecast dataset').selectOption('minute_capture/AAPL');
  await expect(page.getByRole('alert')).toContainText('Model uses 1d bars');
  await expect(page.getByRole('button', { name: 'Generate forecast' })).toBeDisabled();
});

test('displays exact forecast evidence, then clears it when inputs change', async ({ page }) => {
  // Browser rendering contract only; CLI integration trains a real model and
  // exercises the production inference service without mocked predictions.
  await page.route('**/api/datasets', route => route.fulfill({ json: [{ dataset: 'historical', symbol: 'AAPL', interval: '1d', source: 'synthetic', bars: 1000 }] }));
  await page.route('**/api/forecast?*', route => route.fulfill({ json: {
    model: 'inspector_demo', dataset: 'historical', symbol: 'AAPL', interval: '1d', source: 'synthetic',
    as_of_ms: 1704384000000, available_at_ms: 1704384000500, generated_at_ms: Date.now(),
    horizon_bars: 1, last_close: 100, predicted_log_return: Math.log(1.01), predicted_return: 0.01,
    implied_close: 101, dataset_sha256: 'a'.repeat(64), model_package_sha256: 'b'.repeat(64),
    warnings: ['Historical data: not the current market.', 'Synthetic or fixture data: use only to verify the workflow.'],
  } }));
  await page.goto('/forecast');
  await page.getByLabel('Forecast model').selectOption('inspector_demo');
  await page.getByLabel('Forecast dataset').selectOption('historical/AAPL');
  await page.getByRole('button', { name: 'Generate forecast' }).click();
  const result = page.getByRole('region', { name: 'Forecast result' });
  await expect(result).toContainText('1.00%');
  await expect(result).toContainText('101.00');
  await expect(result).toContainText('Historical data');
  await expect(result).toContainText('2024-01-04');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBeTruthy();
  await page.getByLabel('Forecast model').selectOption('');
  await expect(result).toHaveCount(0);
});

test('actual Rust endpoint fails clearly for missing dataset instead of inventing a forecast', async ({ page }) => {
  await page.route('**/api/datasets', route => route.fulfill({ json: [{ dataset: 'missing', symbol: 'AAPL', interval: '1d', source: 'csv', bars: 1000 }] }));
  await page.goto('/forecast');
  await page.getByLabel('Forecast model').selectOption('inspector_demo');
  await page.getByLabel('Forecast dataset').selectOption('missing/AAPL');
  await page.getByRole('button', { name: 'Generate forecast' }).click();
  await expect(page.getByRole('heading', { name: 'Forecast unavailable' })).toBeVisible();
  await expect(page.getByRole('alert')).toContainText('422:');
  await expect(page.getByRole('region', { name: 'Forecast result' })).toHaveCount(0);
});

test('fresh mode explains missing credentials and refuses demo models', async ({ page }) => {
  await page.route('**/api/market/status', route => route.fulfill({ json: { provider: 'Twelve Data', configured: false, message: 'Set TWELVE_DATA_API_KEY on the backend.' } }));
  await page.route('**/api/datasets', route => route.fulfill({ json: [{ dataset:'historical',symbol:'AAPL',interval:'1d',source:'synthetic',bars:1000 }] }));
  await page.goto('/forecast');
  await page.getByLabel('Forecast data mode').selectOption('fresh');
  await page.getByLabel('Forecast model').selectOption('inspector_demo');
  await page.getByLabel('Forecast dataset').selectOption('historical/AAPL');
  await expect(page.getByText('Set TWELVE_DATA_API_KEY on the backend.')).toBeVisible();
  await expect(page.getByText('Demo and legacy models support recorded mode.',{exact:false})).toBeVisible();
  await expect(page.getByRole('button',{name:'Generate forecast'})).toBeDisabled();
  await page.getByLabel('Forecast data mode').selectOption('recorded');
  await expect(page.getByRole('button',{name:'Generate forecast'})).toBeEnabled();
});

test('fresh mode carries selection, displays currency and clears stale output after provider failure', async ({ page }) => {
  const market = {provider_symbol:'INFY',exchange:'NSE',currency:'INR',timezone:'Asia/Kolkata',instrument_type:'Common Stock',periods_per_year:252};
  const source='twelve_data:unadjusted:rest_snapshot';
  await page.route('**/api/market/status',route=>route.fulfill({json:{provider:'Twelve Data',configured:true,message:'Credential configured; exchange access unverified.'}}));
  await page.route('**/api/models',route=>route.fulfill({json:[{artifact_id:'global_model',onnx_present:true,metadata:{market,source,training_symbol:'INFY_NSE',bar_interval:'1d'}}]}));
  await page.route('**/api/datasets',route=>route.fulfill({json:[{dataset:'global_history',symbol:'INFY_NSE',interval:'1d',source,market,bars:1000}]}));
  let fail = false;
  await page.route('**/api/forecast?*',route=> {
    expect(new URL(route.request().url()).searchParams.get('fresh')).toBe('true');
    return fail ? route.fulfill({status:422,json:{message:'Latest completed market bar is stale'}}) : route.fulfill({json:{model:'global_model',dataset:'global_history',symbol:'INFY_NSE',interval:'1d',source,market,input_mode:'fresh',feed_latency:'unknown',fetched_at_ms:1704384000000,data_age_ms:60000,as_of_ms:1704383940000,available_at_ms:1704383940000,generated_at_ms:1704384000000,horizon_bars:1,last_close:100,predicted_return:0.01,implied_close:101,warnings:['Feed delay is not independently verified.']}});
  });
  await page.goto('/forecast');
  await page.getByLabel('Forecast data mode').selectOption('fresh');
  await page.getByLabel('Forecast model').selectOption('global_model');
  await page.getByLabel('Forecast dataset').selectOption('global_history/INFY_NSE');
  await page.getByRole('button',{name:'Generate forecast'}).click();
  await expect(page.getByRole('region',{name:'Forecast result'})).toContainText('Currency: INR');
  await expect(page.getByRole('region',{name:'Forecast result'})).toContainText('Feed delay: unknown');
  fail=true;
  await page.getByRole('button',{name:'Generate forecast'}).click();
  await expect(page.getByRole('alert')).toContainText('stale');
  await expect(page.getByRole('region',{name:'Forecast result'})).toHaveCount(0);
});
