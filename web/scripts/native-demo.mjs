import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, dirname, basename } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const repo = fileURLToPath(new URL('../../', import.meta.url));
function run(command, args, cwd = repo) {
  const result = spawnSync(command, args, { cwd, stdio: 'inherit' });
  if (result.error || result.status !== 0)
    throw new Error(`Could not run ${basename(command)}. Check that it is installed and dependencies are available.`);
}
export async function startNativeDemo({ build = true, port = 0, binary = process.env.QUANTCTL_BIN,
  python = process.env.PYTHON ?? 'python', onWorkspace = () => {} } = {}) {
  if (!Number.isInteger(port) || port < 0 || port > 65535) throw new Error('Port must be an integer from 0 to 65535.');
  if (build) {
    if (!process.env.npm_execpath) throw new Error('Start with npm run demo from the web directory.');
    run(process.execPath, [process.env.npm_execpath, 'run', 'build'], resolve(repo, 'web'));
    if (!binary) run('cargo', ['build', '--locked', '--manifest-path', resolve(repo, 'rust/Cargo.toml'), '--bin', 'quantctl']);
  }
  binary ??= resolve(repo, 'rust/target/debug/quantctl' + (process.platform === 'win32' ? '.exe' : ''));
  const workspace = mkdtempSync(resolve(tmpdir(), 'lstm-native-'));
  let child, closed, stopping;
  const stop = () => stopping ??= (async () => {
    if (child?.pid && child.exitCode === null && child.signalCode === null) {
      child.kill('SIGTERM');
      const timer = setTimeout(() => child.kill('SIGKILL'), 5000);
      try { await closed; } finally { clearTimeout(timer); }
    }
    if (dirname(workspace) !== resolve(tmpdir()) || !basename(workspace).startsWith('lstm-native-'))
      throw new Error('Refusing to remove a workspace outside the demo temporary directory.');
    rmSync(workspace, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
  })();
  try {
    onWorkspace(workspace);
    run(python, [resolve(repo, 'scripts/deployment_bundle.py'), 'extract', '--bundle',
      resolve(repo, 'deploy/artifacts.tar.gz'), '--output', workspace]);
    const env = { ...process.env, QUANTCTL_ALLOW_NETWORK: '0' };
    for (const key of ['QUANTCTL_LIVE_SYMBOLS', 'APCA_API_KEY_ID', 'APCA_API_SECRET_KEY', 'TWELVE_DATA_API_KEY', 'QUANTCTL_BACKEND_TOKEN']) delete env[key];
    child = spawn(binary, ['serve', '--port', String(port), '--root', workspace], { cwd: repo, env, stdio: ['ignore', 'pipe', 'pipe'] });
    closed = new Promise(resolveClose => {
      child.once('error', () => resolveClose(1));
      child.once('close', code => resolveClose(code));
    });
    const url = await new Promise((resolveUrl, reject) => {
      let output = '';
      const timer = setTimeout(() => reject(new Error('Local server did not start within 30 seconds.')), 30_000);
      const finish = (error, value) => { clearTimeout(timer); error ? reject(error) : resolveUrl(value); };
      child.once('error', () => finish(new Error('Could not start quantctl. Run npm run demo to build it, or set QUANTCTL_BIN.')));
      child.once('exit', code => finish(new Error(`Local server exited during startup (code ${code}).`)));
      child.stderr.on('data', () => {}); // Drain logs; never echo inherited configuration.
      child.stdout.on('data', data => {
        output = (output + data).slice(-8192);
        const match = output.match(/Research inspector: http:\/\/127\.0\.0\.1:(\d+)/);
        if (match) finish(null, 'http://127.0.0.1:' + match[1]);
      });
    });
    return { url, workspace, stop, closed };
  } catch (error) { await stop(); throw error; }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  let demo;
  let interrupted = false;
  try {
    const args = process.argv.slice(2);
    if (args.length && !(args.length === 2 && args[0] === '--port' && /^\d+$/.test(args[1])))
      throw new Error('Usage: npm run demo -- [--port 8788]');
    demo = await startNativeDemo({ port: args.length ? Number(args[1]) : 0 });
    const shutdown = () => { interrupted = true; void demo.stop(); };
    process.once('SIGINT', shutdown);
    process.once('SIGTERM', shutdown);
    console.log(`Local research app: ${demo.url}\nUses the bundled synthetic dataset and real ONNX inference. Live feeds are disabled.\nPress Ctrl+C to stop and remove this temporary workspace.`);
    const code = await demo.closed;
    if (code && !interrupted) process.exitCode = code;
  } catch (error) { console.error(error.message); process.exitCode = 1; }
  finally { await demo?.stop(); }
}
