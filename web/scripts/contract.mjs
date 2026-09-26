import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import openapiTS, { astToString } from 'openapi-typescript';
const schema = execFileSync('cargo', ['run', '--quiet', '--manifest-path', '../rust/Cargo.toml', '-p', 'quant_api', '--example', 'openapi'], { encoding: 'utf8', maxBuffer: 8e6 });
const spec = JSON.parse(schema);
const generated = astToString(await openapiTS(spec));
mkdirSync('src/api', { recursive: true });
if (process.argv.includes('--check')) {
  if (readFileSync('src/api/generated.ts', 'utf8') !== generated || readFileSync('../Docs/openapi.json', 'utf8') !== schema) {
    throw new Error('API contract is stale. Run npm run contract and commit both generated files.');
  }
} else {
  writeFileSync('src/api/generated.ts', generated);
  writeFileSync('../Docs/openapi.json', schema);
}
