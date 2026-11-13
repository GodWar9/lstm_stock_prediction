import { useState } from 'react';
import { useMutation, useQuery } from '@tanstack/react-query';
import { json, object, type Model, type Run } from '../api/client';
import type { components } from '../api/generated';
type Dataset = components['schemas']['DatasetChoice'];
type Forecast = components['schemas']['Forecast'];
export function ResearchPanel() {
  const research = useQuery({ queryKey: ['dashboard-research'], queryFn: async () => {
    // The hosted artifact service has one worker; serialize independent reads.
    const models = await json<Model[]>('/api/models');
    const datasets = await json<Dataset[]>('/api/datasets');
    const runs = await json<Run[]>('/api/runs');
    return { models, datasets, runs };
  }, staleTime: 60000, refetchOnWindowFocus: false, retry: 2 });
  const models = { data: research.data?.models, isPending: research.isPending, error: research.error };
  const datasets = { data: research.data?.datasets, isPending: research.isPending, error: research.error };
  const runs = { data: research.data?.runs, error: research.error };
  const [chosen, choose] = useState('');
  const pairs = (models.data ?? []).flatMap(m => (datasets.data ?? []).filter(d => {
    const meta = object(m.metadata);
    return m.onnx_present && (meta.bar_interval ?? '1d') === d.interval && (meta.training_symbol ? meta.training_symbol === d.symbol : meta.training_dataset_version === d.dataset) && (!meta.source || meta.source === d.source);
  }).map(d => ({ m, d, key: `${m.artifact_id}/${d.dataset}/${d.symbol}` })));
  const pair = pairs.find(p => p.key === chosen) ?? pairs[0];
  const forecast = useMutation({ mutationFn: (p: typeof pairs[number]) => json<Forecast>('/api/forecast?' + new URLSearchParams({ model: p.m.artifact_id, dataset: p.d.dataset, symbol: p.d.symbol })), retry: false });
  const result = forecast.data;
  const validResult = result && pair && result.model === pair.m.artifact_id && result.dataset === pair.d.dataset && result.symbol === pair.d.symbol ? result : undefined;
  const loading = models.isPending || datasets.isPending;
  const error = models.error ?? datasets.error ?? runs.error;
  const num = (v: number) => v.toLocaleString('en-US', { maximumFractionDigits: 4 });
  return <section className="terminal-panel research-panel"><div className="section-heading"><div><span className="terminal-eyebrow">LSTM INFERENCE</span><h3>Forecast workspace</h3></div><a href="/forecast">Full forecast controls ↗</a></div>
    <div className="research-counts"><span><b>{models.data?.filter(m => m.onnx_present).length ?? '—'}</b> inference models</span><span><b>{datasets.data?.length ?? '—'}</b> datasets</span><span><b>{runs.data?.filter(r => r.split === 'test').length ?? '—'}</b> held-out runs</span></div>
    {error && <p role="alert">Research backend unavailable: {error.message} <button onClick={() => void research.refetch()}>Retry research inputs</button></p>}
    {loading ? <p role="status">Loading model artifacts…</p> : error ? null : !pairs.length ? <p>No compatible model and dataset are deployed. Market quotes alone cannot produce an LSTM forecast.</p> : <><div className="research-controls"><label>Model & research dataset<select aria-label="Dashboard forecast dataset" value={pair?.key ?? ''} disabled={forecast.isPending} onChange={e => { choose(e.target.value); forecast.reset(); }}>{pairs.map(p => <option key={p.key} value={p.key}>{p.d.symbol} · {p.d.source} · {p.m.artifact_id}</option>)}</select></label><button className="primary-action" disabled={!pair || forecast.isPending} onClick={() => pair && forecast.mutate(pair)}>{forecast.isPending ? 'Running inference…' : 'Generate forecast'}</button></div><p className="panel-caption">Recorded research · {pair?.d.bars.toLocaleString()} {pair?.d.interval} bars · Source: {pair?.d.source}. {pair?.d.source === 'synthetic' ? 'Synthetic demonstration, not a live-market prediction.' : 'This dataset is separate from the market instrument selected above.'}</p></>}
    {forecast.error && <p role="alert">{forecast.error.message}</p>}
    {validResult && <div className="dashboard-forecast" aria-label="Dashboard forecast result"><div><span>Last recorded close</span><b>{num(validResult.last_close)}</b></div><div><span>Predicted return</span><b className={validResult.predicted_return >= 0 ? 'bids' : 'asks'}>{num(validResult.predicted_return * 100)}%</b></div><div><span>Implied close</span><b>{num(validResult.implied_close)}</b></div><div><span>Forecast horizon</span><b>{validResult.horizon_bars} × {validResult.interval}</b></div><p>{validResult.symbol} · Data as of {new Date(validResult.as_of_ms).toISOString()} · {validResult.input_mode}</p>{validResult.warnings.map(w => <p key={w}>{w}</p>)}</div>}
  </section>;
}
