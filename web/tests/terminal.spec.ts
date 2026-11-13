import { test, expect } from '@playwright/test';

test('free forex renders daily reference analytics without fabricated candles', async ({ page }) => {
  await page.route('https://api.exchange.coinbase.com/**', route => route.fulfill({status:503}));
  await page.routeWebSocket('wss://ws-feed.exchange.coinbase.com', () => {});
  await page.route('**/api/markets/catalog?*', route => route.fulfill({json:{total:1,catalogTotal:1,items:[{symbol:'EUR/INR',name:'Euro / Indian Rupee',exchange:'Forex',currency:'INR',country:'',type:'Physical Currency'}],exchanges:['Forex'],pricesConfigured:false}}));
  await page.route('**/api/markets/reference?*', route => {
    const symbol = new URL(route.request().url()).searchParams.get('symbol');
    if (symbol === 'EUR/INR') return route.fulfill({status:502,json:{message:'Reference provider unavailable'}});
    return route.fulfill({json:{symbol:'EUR/USD',currency:'USD',source:'Frankfurter blended reference rates',sourceUrl:'https://frankfurter.dev/',asOf:'2026-11-12',stale:false,points:[{date:'2026-11-10',rate:1.1},{date:'2026-11-12',rate:1.21}],metrics:{last:1.21,changePct:10,low30:1.1,high30:1.21,mean30:1.155,observations:2,windowObservations:2}}});
  });
  await page.goto('/terminal');
  await page.getByRole('button',{name:'Forex',exact:true}).click();
  await expect(page.getByRole('img',{name:/EUR\/USD daily reference rates/})).toBeVisible();
  await expect(page.getByText('10%',{exact:true})).toBeVisible();
  await expect(page.getByText('Observation date: 2026-11-12.',{exact:false})).toBeVisible();
  await expect(page.getByRole('button',{name:'Candles',exact:true})).toHaveCount(0);
  await page.getByLabel('Reference history range').selectOption('365');
  await page.getByText('View reference observations',{exact:true}).click();
  await expect(page.getByRole('cell',{name:'2026-11-10',exact:true})).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.getByRole('button',{name:/EUR\/INR Euro/}).click();
  await expect(page.getByRole('alert').filter({hasText:'Free reference history unavailable for EUR/INR'})).toBeVisible();
  await expect(page.getByRole('img',{name:/EUR\/USD daily reference rates/})).toHaveCount(0);
});
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
