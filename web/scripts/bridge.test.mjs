import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { createBridge } from '../api/bridge.js';

const token = 'test-token-'.repeat(5);
const env = { QUANTCTL_BACKEND_URL: 'https://backend.example', QUANTCTL_BACKEND_TOKEN: token };
async function server(t, options) {
  const app = createServer(createBridge(options));
  app.listen(0, '127.0.0.1');
  await once(app, 'listening');
  t.after(() => { app.closeAllConnections(); app.close(); });
  return 'http://127.0.0.1:' + app.address().port;
}
test('injects only the server credential and preserves binary bytes, query and pagination headers', async t => {
  let seen;
  const bytes = new Uint8Array([0, 255, 1, 13, 10, 65]);
  const url = await server(t, { env, fetchImpl: async (url, options) => {
    seen = { url: url.toString(), options };
    return new Response(bytes, { headers: { 'content-type': 'application/vnd.apache.arrow.file', 'x-total-count': '400', 'set-cookie': 'private=secret' } });
  }});
  const response = await fetch(url + '/api/bridge?__path=runs/run-1/equity.arrow&max_points=100', {
    headers: { authorization: 'Bearer user-secret', cookie: 'private-user-cookie' },
  });
  assert.deepEqual(new Uint8Array(await response.arrayBuffer()), bytes);
  assert.equal(seen.url, 'https://backend.example/api/runs/run-1/equity.arrow?max_points=100');
  assert.deepEqual(seen.options.headers, { authorization: 'Bearer ' + token, accept: '*/*' });
  assert.equal(seen.options.redirect, 'error');
  assert.equal(response.headers.get('x-total-count'), '400');
  assert.equal(response.headers.get('set-cookie'), null);
  assert.equal(response.headers.get('cache-control'), 'no-store');
});
test('rejects unknown routes and mutation requests without contacting backend', async t => {
  const url = await server(t, { env, fetchImpl: () => { assert.fail('Unexpected upstream request'); } });
  for (const path of ['/api/.env', '/api/bridge?__path=../secret', '/api/bridge?__path=runs&__path=models', '/api/bridge?__path=https://evil.test', '/api/bridge?__path=runs/a/%2e%2e/secret']) {
    assert.ok([400,404].includes((await fetch(url + path)).status), path);
  }
  assert.equal((await fetch(url + '/api/runs', { method: 'POST', body: 'x' })).status, 405);
});
test('missing, insecure, or ambiguous backend settings fail closed', async t => {
  for (const value of [undefined, 'http://public.example', 'https://user:pass@backend.example', 'https://backend.example/private']) {
    const url = await server(t, { env: { ...env, QUANTCTL_BACKEND_URL: value } });
    assert.equal((await fetch(url + '/api/runs')).status, 503);
  }
  const url = await server(t, { env: { ...env, QUANTCTL_BACKEND_TOKEN: 'short' } });
  assert.equal((await fetch(url + '/api/runs')).status, 503);
});
test('preserves backend error status and hides backend authentication details', async t => {
  const url = await server(t, { env, fetchImpl: async url => new Response(JSON.stringify({message:'Missing dataset'}), {status: url.pathname === '/api/runs' ? 422 : 401, headers:{'content-type':'application/json'}}) });
  assert.equal((await fetch(url + '/api/runs')).status, 422);
  const response = await fetch(url + '/api/models');
  assert.equal(response.status, 502);
  assert.equal((await response.json()).code, 'backend_auth_failed');
});
test('forwards SSE immediately and aborts upstream after browser disconnect', async t => {
  let signal;
  const url = await server(t, { env, fetchImpl: async (_, options) => {
    signal = options.signal;
    return new Response(new ReadableStream({ start(c) { c.enqueue(new TextEncoder().encode('event: live\ndata: {"status":"disabled"}\n\n')); } }), {headers:{'content-type':'text/event-stream'}});
  }});
  const controller = new AbortController();
  const response = await fetch(url + '/api/live/events', { signal: controller.signal });
  const reader = response.body.getReader();
  assert.match(new TextDecoder().decode((await reader.read()).value), /event: live/);
  controller.abort();
  await new Promise(resolve => setTimeout(resolve, 40));
  assert.equal(signal.aborted, true);
});
test('times out a stalled backend and limits oversized responses', async t => {
  const url = await server(t, { env, timeoutMs: 30, fetchImpl: (_, {signal}) => new Promise((_, reject) => signal.addEventListener('abort', () => reject(new Error('timeout')))) });
  assert.equal((await fetch(url + '/api/runs')).status, 504);
  const large = await server(t, { env, fetchImpl: async () => new Response(new Uint8Array(4 * 1024 * 1024 + 1)) });
  assert.equal((await fetch(large + '/api/runs')).status, 502);
});

test('global market status and fresh forecast queries pass through the server bridge', async t => {
  const seen=[];
  const url=await server(t,{env,fetchImpl:async url=>{seen.push(url.toString());return new Response('{}',{headers:{'content-type':'application/json'}});}});
  assert.equal((await fetch(url+'/api/bridge?__path=market/status')).status,200);
  assert.equal((await fetch(url+'/api/bridge?__path=forecast&model=m&dataset=d&symbol=INFY_NSE&fresh=true')).status,200);
  assert.equal(seen[0],'https://backend.example/api/market/status');
  assert.match(seen[1],/fresh=true/);
});
