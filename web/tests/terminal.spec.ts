import { test, expect } from '@playwright/test';
test('terminal acquires public snapshots, streams quotes and switches products', async ({ page }) => {
  await page.route('https://api.exchange.coinbase.com/**', route => {
    const url = route.request().url();
    const body = url.endsWith('/products') ? ['BTC-USD', 'ETH-USD', 'SOL-USD', 'LINK-USD', 'ETH-EUR'].map(id => ({ id, base_currency:id.split('-')[0], quote_currency:id.split('-')[1], status:'online', display_name:id })) : url.includes('/candles') ? [[Math.floor(Date.now()/60000)*60, 99, 102, 100, 101, 4]] : url.includes('/book') ? { bids: [['100','2']], asks: [['101','3']] } : [{ trade_id: 1, price: '100', size: '2', side: 'buy', time: new Date().toISOString() }];
    return route.fulfill({ json: body });
  });
  await page.routeWebSocket('wss://ws-feed.exchange.coinbase.com', ws => {
    ws.onMessage(() => {
      for (const product_id of ['BTC-USD', 'ETH-USD', 'SOL-USD']) ws.send(JSON.stringify({ type: 'ticker', product_id, price: '101', open_24h: '100', high_24h: '102', low_24h: '99', volume_24h: '500', time: new Date().toISOString() }));
    });
  });
  await page.route('**/api/markets/catalog?*', route => route.fulfill({json:{total:1,catalogTotal:1,items:[{symbol:'AAPL',name:'Apple Inc',exchange:'NASDAQ',currency:'USD',country:'United States',type:'Common Stock'}],exchanges:['NASDAQ'],pricesConfigured:false}}));
  await page.route('**/api/markets/status', route => route.fulfill({json:{configured:false}}));
  await page.goto('/terminal');
  await expect(page.getByRole('heading', { name: /Market terminal/ })).toBeVisible();
  await expect(page.getByRole('status').filter({ hasText: 'Live' })).toBeVisible();
  await expect(page.getByText('Spread $1.00', {exact:false})).toBeVisible();
  await page.getByRole('button', { name: /ETH-USD/ }).click();
  await expect(page.getByRole('img', { name: /ETH-USD 1m/ })).toBeVisible();
  await page.getByRole('button', {name:'1h', exact:true}).click();
  await expect(page.getByRole('img', {name:/ETH-USD 1h/})).toBeVisible();
  await page.getByRole('textbox', {name:'Search markets'}).fill('LINK');
  await page.getByRole('button', {name:/LINK-USD/}).click();
  await expect(page.getByRole('img', {name:/LINK-USD 1m/})).toBeVisible();
  await page.getByRole('button', {name:'Stocks',exact:true}).click();
  await page.getByRole('button', {name:/AAPL Apple Inc/}).click();
  await expect(page.getByRole('heading', {name:'AAPL', exact:true}).first()).toBeVisible();
  await expect(page.getByText('Price/history feed not connected.',{exact:false})).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});
test('terminal shows feed failures without fabricated data', async ({ page }) => {
  await page.route('https://api.exchange.coinbase.com/**', route => route.fulfill({ status: 503 }));
  await page.routeWebSocket('wss://ws-feed.exchange.coinbase.com', () => {});
  await page.route('**/api/markets/catalog?*', route => route.fulfill({json:{total:1,catalogTotal:1,items:[{symbol:'AAPL',name:'Apple Inc',exchange:'NASDAQ',currency:'USD',country:'United States',type:'Common Stock'}],exchanges:['NASDAQ'],pricesConfigured:false}}));
  await page.route('**/api/markets/status', route => route.fulfill({json:{configured:false}}));
  await page.goto('/terminal');
  await expect(page.getByText('Feed unavailable; retrying', { exact: false }).first()).toBeVisible();
  await expect(page.getByText('No observations in this range.')).toBeVisible();
});
