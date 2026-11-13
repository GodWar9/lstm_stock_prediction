import { mkdir, writeFile } from 'node:fs/promises';
import { gzipSync } from 'node:zlib';
const catalogs = {};
for (const [asset, endpoint] of [['stocks', 'stocks'], ['forex', 'forex_pairs']]) {
  const response = await fetch('https://api.twelvedata.com/' + endpoint, { signal: AbortSignal.timeout(120000) });
  if (!response.ok) throw new Error('Catalog request failed');
  const raw = await response.json();
  if (!Array.isArray(raw.data) || raw.data.length < 100) throw new Error('Unexpected catalog');
  catalogs[asset] = raw.data.filter(r => typeof r.symbol === 'string').map(r => ({ symbol: r.symbol, name: r.name ?? `${r.currency_base ?? ''} / ${r.currency_quote ?? ''}`, exchange: r.exchange ?? 'Forex', currency: r.currency ?? r.symbol.split('/')[1] ?? '', country: r.country ?? '', type: r.type ?? 'Physical Currency' }));
  console.log(asset + ': ' + catalogs[asset].length + ' listings');
}
const artifact = { source: 'Twelve Data public reference catalogues', fetchedAt: new Date().toISOString(), catalogs };
await mkdir(new URL('../data/', import.meta.url), { recursive: true });
const bytes = gzipSync(JSON.stringify(artifact), { level: 9 });
await writeFile(new URL('../data/securities.json.gz', import.meta.url), bytes);
console.log('Saved compressed reference catalogue: ' + bytes.length + ' bytes');
