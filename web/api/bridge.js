import { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';

// Only this server module reads credentials. Never prefix these settings with VITE_.
const ID = '[A-Za-z0-9_-]{1,128}';
const allowed = new RegExp('^/api/(?:health|runs|models|datasets|forecast|events|live(?:/events|/ready)?|openapi\\.json|models/' + ID + '|runs/' + ID + '/(?:manifest|equity\\.arrow|trades\\.arrow|signals\\.arrow|simulation/bands\\.arrow|validation|simulation\\.json|report\\.json|benchmark\\.arrow|positions\\.arrow|signal_outcomes\\.arrow|risk\\.json))$');
const forwardedHeaders = ['content-type', 'x-total-count', 'x-returned-count', 'x-offset', 'x-sampled'];
const maxResponseBytes = 4 * 1024 * 1024;

function fail(res, status, code, message) {
  res.writeHead(status, { 'content-type': 'application/json', 'cache-control': 'no-store' });
  res.end(JSON.stringify({ code, message }));
}

export function createBridge({ env = process.env, fetchImpl = fetch, timeoutMs = 110_000 } = {}) {
  return async function bridge(req, res) {
    if (req.method !== 'GET') {
      res.setHeader('allow', 'GET');
      return fail(res, 405, 'method_not_allowed', 'This application provides read-only research endpoints.');
    }
    // Do not parse arbitrary user input as an upstream URL.
    const raw = req.url ?? '';
    if (!raw.startsWith('/') || raw.startsWith('//') || raw.includes('\\') || raw.length > 4096) {
      return fail(res, 400, 'invalid_path', 'Invalid API path.');
    }
    const requestUrl = new URL(raw, 'http://bridge.local');
    let path = raw.split('?')[0];
    const query = requestUrl.searchParams;
    // Vercel rewrite destination; tests also exercise the original incoming path.
    if (path === '/api/bridge') {
      if (query.getAll('__path').length !== 1) return fail(res, 400, 'invalid_path', 'Missing API path.');
      path = '/api/' + query.get('__path');
    }
    query.delete('__path');
    if (!allowed.test(path)) return fail(res, 404, 'not_found', 'Unknown API endpoint.');
    let backend;
    const token = env.QUANTCTL_BACKEND_TOKEN;
    try {
      backend = new URL(env.QUANTCTL_BACKEND_URL);
      const loopback = ['localhost', '127.0.0.1', '[::1]'].includes(backend.hostname);
      if ((backend.protocol !== 'https:' && !(backend.protocol === 'http:' && loopback))
          || backend.username || backend.password || backend.search || backend.hash || backend.pathname !== '/'
          || typeof token !== 'string' || !/^[\x21-\x7e]{32,256}$/.test(token)) throw new Error();
    } catch {
      return fail(res, 503, 'backend_not_configured', 'The research backend has not been configured.');
    }
    const upstreamUrl = new URL(path, backend);
    upstreamUrl.search = query.toString();
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), timeoutMs);
    const disconnect = () => controller.abort();
    res.on('close', disconnect);
    try {
      const upstream = await fetchImpl(upstreamUrl, {
        headers: { authorization: 'Bearer ' + token, accept: '*/*' },
        signal: controller.signal, redirect: 'error',
      });
      if (upstream.status === 401 || upstream.status === 403) {
        await upstream.body?.cancel();
        return fail(res, 502, 'backend_auth_failed', 'The research backend connection needs administrator attention.');
      }
      const headers = { 'cache-control': 'no-store', 'x-content-type-options': 'nosniff' };
      for (const name of forwardedHeaders) {
        if (upstream.headers.has(name)) headers[name] = upstream.headers.get(name);
      }
      const isStream = upstream.headers.get('content-type')?.startsWith('text/event-stream');
      if (isStream) {
        res.writeHead(upstream.status, { ...headers, 'x-accel-buffering': 'no' });
        res.flushHeaders();
        if (upstream.body) await pipeline(Readable.fromWeb(upstream.body), res, { signal: controller.signal });
        else res.end();
      } else {
        // Stay below the function payload limit, including when Content-Length is absent.
        const chunks = [];
        let size = 0;
        if (upstream.body) {
          for await (const chunk of upstream.body) {
            size += chunk.length;
            if (size > maxResponseBytes) {
              controller.abort();
              return fail(res, 502, 'response_too_large', 'Request a smaller page or fewer chart points.');
            }
            chunks.push(Buffer.from(chunk));
          }
        }
        res.writeHead(upstream.status, headers);
        res.end(Buffer.concat(chunks));
      }
    } catch {
      if (!res.destroyed) {
        if (res.headersSent) res.end(); // EventSource reconnects after a bounded stream.
        else fail(res, controller.signal.aborted ? 504 : 502, 'backend_unavailable',
          'The research backend is starting or unavailable. Please retry shortly.');
      }
    } finally {
      clearTimeout(timer);
      controller.abort();
      res.off('close', disconnect);
    }
  };
}
export default createBridge();
