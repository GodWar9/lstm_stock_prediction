import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createMarkets } from '../api/markets.mjs';
async function call(handler, url, method = 'GET') {
  let status;
  let body;
  await handler({ url, method }, { writeHead(code) { status = code; }, end(text) { body = JSON.parse(text); } });
  return { status, body };
}
const history = { meta: { symbol: 'AAPL', exchange: 'NASDAQ', currency: 'USD' }, values: [{ datetime: '2026-11-12', low: '99', high: '102', open: '100', close: '101', volume: '20' }] };
test('packaged global catalog works offline with a dated source', async () => {
  const api = createMarkets({env:{},fetchImpl:()=>assert.fail('catalog should be packaged')});
  const stocks = await call(api,'/api/markets/catalog?asset=stocks&q=AAPL');
  assert.equal(stocks.status,200); assert.ok(stocks.body.catalogTotal > 200000);
  assert.ok(stocks.body.items.some(r=>r.symbol === 'AAPL')); assert.ok(stocks.body.asOf);
  const forex = await call(api,'/api/markets/catalog?asset=forex&q=EUR/USD');
  assert.equal(forex.body.items[0].symbol,'EUR/USD');
});
test('global catalogue searches, paginates and caches without a key', async () => {
  let requests = 0;
  const api = createMarkets({ catalogFile: null, env: {}, fetchImpl: async url => { requests++; assert.equal(url.pathname, '/stocks'); return Response.json({data:Array.from({length:70},(_,i)=>({symbol:`A${i}`,name:'Global security',exchange:'XNAS',currency:'USD'}))}); } });
  const first = await call(api, '/api/markets/catalog?asset=stocks');
  assert.equal(first.body.items.length,50); assert.equal(first.body.total,70);
  assert.equal((await call(api, '/api/markets/catalog?asset=stocks&offset=50')).body.items.length,20);
  assert.equal((await call(api, '/api/markets/catalog?asset=stocks&q=A69')).body.items[0].symbol,'A69');
  assert.equal(requests,1);
});
test('missing key or display permission never requests paid data', async () => {
  for (const env of [{}, {TWELVE_DATA_API_KEY:'secret'}]) {
    const api = createMarkets({ catalogFile: null,env,fetchImpl:()=>assert.fail('must not contact provider')});
    assert.equal((await call(api,'/api/markets/snapshot?symbol=AAPL')).status,503);
  }
});
test('snapshot sends credentials only in headers, coalesces requests and validates OHLC', async () => {
  let requests=0;
  const api=createMarkets({ catalogFile: null,env:{TWELVE_DATA_API_KEY:'secret',TWELVE_DATA_PUBLIC_DISPLAY:'1'},fetchImpl:async(url,options)=>{
    requests++; assert.equal(options.headers.Authorization,'apikey secret'); assert.ok(!url.href.includes('secret')); return Response.json(history);
  }});
  const results=await Promise.all([call(api,'/api/markets/snapshot?symbol=AAPL'),call(api,'/api/markets/snapshot?symbol=AAPL')]);
  assert.equal(requests,1); assert.equal(results[0].body.bars[0][4],101); assert.ok(!JSON.stringify(results).includes('secret'));
});
test('provider errors are redacted and unsupported paths never call upstream',async()=>{
  const api=createMarkets({ catalogFile: null,env:{TWELVE_DATA_API_KEY:'secret',TWELVE_DATA_PUBLIC_DISPLAY:'1'},fetchImpl:async()=>Response.json({status:'error',message:'secret'})});
  const r=await call(api,'/api/markets/snapshot?symbol=AAPL');assert.equal(r.status,502);assert.ok(!JSON.stringify(r).includes('secret'));
  assert.equal((await call(api,'/api/markets/snapshot?symbol=https://bad.com&interval=bad')).status,400);
  assert.equal((await call(api,'/api/markets/private')).status,404);
  assert.equal((await call(api,'/api/markets/status','POST')).status,405);
});
test('paid snapshot budget bounds distinct requests',async()=>{
  const api=createMarkets({ catalogFile: null,env:{TWELVE_DATA_API_KEY:'secret',TWELVE_DATA_PUBLIC_DISPLAY:'1'},fetchImpl:async url=>Response.json({...history,meta:{...history.meta,symbol:url.searchParams.get('symbol')}})});
  for(let i=0;i<4;i++) assert.equal((await call(api,`/api/markets/snapshot?symbol=A${i}`)).status,200);
  assert.equal((await call(api,'/api/markets/snapshot?symbol=LAST')).status,429);
});
