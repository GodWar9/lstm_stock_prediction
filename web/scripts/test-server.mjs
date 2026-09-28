import { cpSync, mkdirSync, mkdtempSync } from 'node:fs';
import { spawn, execFileSync } from 'node:child_process';
import { resolve } from 'node:path';
// Always test current production assets, even when invoked without a prior build.
execFileSync(process.execPath, ['node_modules/typescript/bin/tsc', '-b'], { stdio: 'inherit' });
execFileSync(process.execPath, ['node_modules/vite/bin/vite.js', 'build'], { stdio: 'inherit' });
execFileSync('cargo', ['build', '--quiet', '--locked', '--manifest-path', '../rust/Cargo.toml', '--bin', 'quantctl'], { stdio: 'inherit' });
mkdirSync('.test-workspace', { recursive: true });
const root = mkdtempSync(resolve('.test-workspace/run-'));
mkdirSync(`${root}/reports`, { recursive: true });
cpSync('tests/fixtures/runs', `${root}/reports/runs`, { recursive: true });
cpSync('tests/fixtures/models', `${root}/models`, { recursive: true });
// Never open a real provider connection from deterministic browser tests.
const env = { ...process.env };
delete env.QUANTCTL_LIVE_SYMBOLS;
delete env.APCA_API_KEY_ID;
delete env.APCA_API_SECRET_KEY;
const child = spawn(resolve('../rust/target/debug/quantctl' + (process.platform === 'win32' ? '.exe' : '')), ['serve', '--port', '8788', '--root', root], { stdio: 'inherit', env });
for (const event of ['SIGTERM', 'SIGINT']) process.on(event, () => child.kill(event));
child.on('exit', code => process.exit(code ?? 1));
