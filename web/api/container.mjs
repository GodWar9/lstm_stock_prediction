import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { randomBytes } from 'node:crypto';
import { readFile, stat } from 'node:fs/promises';
import { resolve, sep, extname } from 'node:path';
import { pathToFileURL } from 'node:url';
import { createBridge } from './bridge.js';

const headers = {
  'x-content-type-options': 'nosniff',
  'referrer-policy': 'no-referrer',
  'content-security-policy': "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; worker-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'",
};
const mime = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.css': 'text/css', '.svg': 'image/svg+xml', '.png': 'image/png' };
const pages = new Set(['/', '/forecast', '/live', '/backtest', '/validation', '/models', '/signals', '/risk', '/jobs']);

export async function startContainerApp({ binary = '/usr/local/bin/quantctl', root = '/app',
  assets = '/site', port = Number(process.env.PORT ?? 8080), host = '0.0.0.0' } = {}) {
  if (!Number.isInteger(port) || port < 0 || port > 65535) throw new Error('Invalid public port');
  assets = resolve(assets);
  const token = randomBytes(32).toString('hex');
  const env = { ...process.env, QUANTCTL_BACKEND_TOKEN: token, QUANTCTL_ALLOW_NETWORK: '0' };
  for (const key of ['QUANTCTL_LIVE_SYMBOLS', 'APCA_API_KEY_ID', 'APCA_API_SECRET_KEY', 'TWELVE_DATA_API_KEY']) delete env[key];
  const child = spawn(binary, ['serve', '--remote', '--workers', '1', '--port', '0', '--root', root], {
    cwd: root, env, stdio: ['ignore', 'pipe', 'pipe'],
  });
  const closed = new Promise(resolveClose => {
    child.once('error', () => resolveClose(1));
    child.once('close', code => resolveClose(code));
  });
  let server, stopping;
  const stop = () => stopping ??= (async () => {
    server?.closeAllConnections();
    server?.close();
    if (child.pid && child.exitCode === null && child.signalCode === null) {
      child.kill('SIGTERM');
      const timer = setTimeout(() => child.kill('SIGKILL'), 5000);
      try { await closed; } finally { clearTimeout(timer); }
    }
  })();
  try {
    const backend = await new Promise((resolveBackend, reject) => {
      let output = '';
      const timer = setTimeout(() => reject(new Error('Inference engine startup timed out')), 30_000);
      child.once('error', () => { clearTimeout(timer); reject(new Error('Could not start inference engine')); });
      child.once('exit', () => { clearTimeout(timer); reject(new Error('Inference engine exited during startup')); });
      child.stderr.on('data', () => {}); // Drain logs without exposing inherited configuration.
      child.stdout.on('data', data => {
        output = (output + data).slice(-8192);
        const match = output.match(/Research inspector: http:\/\/0\.0\.0\.0:(\d+)/);
        if (match) { clearTimeout(timer); resolveBackend('http://127.0.0.1:' + match[1]); }
      });
    });
    const bridge = createBridge({ env: { QUANTCTL_BACKEND_URL: backend, QUANTCTL_BACKEND_TOKEN: token } });
    server = createServer(async (req, res) => {
      for (const [key, value] of Object.entries(headers)) res.setHeader(key, value);
      if (req.url?.startsWith('/api/')) return bridge(req, res);
      try {
        if (!['GET', 'HEAD'].includes(req.method)) {
          res.writeHead(405, { allow: 'GET, HEAD' }); return res.end();
        }
        const path = decodeURIComponent(new URL(req.url, 'http://site.local').pathname);
        const file = pages.has(path) ? resolve(assets, 'index.html') : resolve(assets, '.' + path);
        if (path.includes('\\') || !file.startsWith(assets + sep) || !(await stat(file)).isFile()) {
          res.writeHead(404); return res.end('Not found');
        }
        const data = await readFile(file);
        res.writeHead(200, { 'content-type': mime[extname(file)] ?? 'application/octet-stream',
          'cache-control': extname(file) === '.html' ? 'no-cache' : 'public, max-age=31536000, immutable' });
        res.end(req.method === 'HEAD' ? undefined : data);
      } catch {
        res.writeHead(404); res.end('Not found');
      }
    });
    await new Promise((resolveListening, reject) => {
      server.once('error', reject);
      server.listen(port, host, resolveListening);
    });
    child.once('close', () => { server.closeAllConnections(); server.close(); });
    return { url: 'http://127.0.0.1:' + server.address().port, backend, stop, closed };
  } catch (error) { await stop(); throw error; }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  let app, interrupted = false;
  try {
    app = await startContainerApp();
    for (const event of ['SIGINT', 'SIGTERM']) process.once(event, () => { interrupted = true; void app.stop(); });
    console.log('Research website ready on port ' + new URL(app.url).port);
    const code = await app.closed;
    if (code && !interrupted) process.exitCode = 1;
  } catch (error) { console.error(error.message); process.exitCode = 1; }
  finally { await app?.stop(); }
}
