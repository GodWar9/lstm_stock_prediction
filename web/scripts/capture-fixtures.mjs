// Copy actual quantctl outputs. This script never synthesizes metrics or series.
import { readFileSync, readdirSync, mkdirSync, cpSync } from 'node:fs';
const root = '../reports/runs';
const runs = readdirSync(root).flatMap(id => {
  try { return [{ id, manifest: JSON.parse(readFileSync(`${root}/${id}/manifest.json`, 'utf8')) }]; }
  catch { return []; }
}).filter(r => r.manifest.provenance.model_artifact_id === 'inspector_demo').sort((a, b) => b.manifest.created_at.localeCompare(a.manifest.created_at));
for (const kind of ['backtest', 'simulation']) {
  const run = runs.find(r => r.manifest.kind === kind);
  if (!run) throw new Error(`Generate an inspector_demo ${kind} with quantctl first`);
  const destination = `tests/fixtures/runs/${run.id}`;
  mkdirSync(destination, { recursive: true });
  cpSync(`${root}/${run.id}`, destination, { recursive: true });
}
mkdirSync('tests/fixtures/models/inspector_demo', { recursive: true });
for (const file of ['metadata.json', 'validation.json', 'training_log.json']) {
  cpSync(`../models/inspector_demo/${file}`, `tests/fixtures/models/inspector_demo/${file}`);
}
