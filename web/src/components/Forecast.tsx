import { useState } from 'react';
import { useMutation, useQuery } from '@tanstack/react-query';
import { json, object, type Model } from '../api/client';
import type { components } from '../api/generated';
import { number, percent } from '../lib/format';

type Dataset = components['schemas']['DatasetChoice'];
type Result = components['schemas']['Forecast'];

export function Forecast() {
  const models = useQuery({ queryKey: ['models'], queryFn: () => json<Model[]>('/api/models'), staleTime: 0 });
  const datasets = useQuery({ queryKey: ['datasets'], queryFn: () => json<Dataset[]>('/api/datasets'), staleTime: 0 });
  const [modelId, setModelId] = useState('');
  const [datasetKey, setDatasetKey] = useState('');
  const selected = datasets.data?.find(d => `${d.dataset}/${d.symbol}` === datasetKey);
  const model = models.data?.find(m => m.artifact_id === modelId);
  const interval = model ? String(object(model.metadata).bar_interval ?? '1d') : undefined;
  const mismatch = !!selected && !!interval && selected.interval !== interval;
  const forecast = useMutation({ mutationFn: (query: URLSearchParams) => json<Result>(`/api/forecast?${query}`), retry: false });
  const reset = () => forecast.reset();
  const result = forecast.data;
  return <>
    <div className="page-title"><h2>Make a forecast</h2><p>Run the Rust ONNX model on the latest complete feature window in a recorded dataset.</p></div>
    <div className="warning">Recorded-data research. Live prices on the market page are not model inputs here. Check the data timestamp and source before interpreting a forecast.</div>
    <section className="panel"><h3>Forecast inputs</h3>
      {(models.isPending || datasets.isPending) && <p role="status">Loading models and datasets…</p>}
      {(models.error || datasets.error) && <div role="alert">{models.error?.message ?? datasets.error?.message}<button onClick={() => { void models.refetch(); void datasets.refetch(); }}>Reload inputs</button></div>}
      {models.data?.length === 0 && <p>No trained models are available. Train a model with <code>quantctl train</code> before forecasting.</p>}
      {datasets.data?.length === 0 && <p>No recorded datasets are available. Ingest market data with <code>quantctl data ingest</code> or import a completed Alpaca capture with <code>quantctl research --journal …</code>.</p>}
      <div className="forecast-inputs"><label>Model<select aria-label="Forecast model" value={modelId} disabled={forecast.isPending} onChange={e => { setModelId(e.target.value); reset(); }}><option value="">Select a model</option>{models.data?.map(m => <option key={m.artifact_id} value={m.artifact_id} disabled={!m.onnx_present}>{m.artifact_id}{!m.onnx_present ? ' (ONNX missing)' : ''}</option>)}</select></label>
      <label>Dataset and symbol<select aria-label="Forecast dataset" value={datasetKey} disabled={forecast.isPending} onChange={e => { setDatasetKey(e.target.value); reset(); }}><option value="">Select a dataset</option>{datasets.data?.map(d => <option key={`${d.dataset}/${d.symbol}`} value={`${d.dataset}/${d.symbol}`}>{d.symbol} · {d.interval} · {d.dataset} · {d.source}</option>)}</select></label></div>
      {selected && <p>Source: {selected.source}. {selected.bars.toLocaleString()} recorded {selected.interval} bars.</p>}
      {mismatch && <p role="alert">Model uses {interval} bars; this dataset uses {selected?.interval}. Select matching intervals.</p>}
      <button disabled={!model || !selected || mismatch || forecast.isPending} onClick={() => { if (selected) forecast.mutate(new URLSearchParams({ model: modelId, dataset: selected.dataset, symbol: selected.symbol })); }}>{forecast.isPending ? 'Computing forecast…' : 'Generate forecast'}</button>
      {forecast.isPending && <p role="status">Verifying artifact hashes, building features and running inference…</p>}
      {forecast.error && <div role="alert"><h3>Forecast unavailable</h3><p>{forecast.error.message}</p><p>Use a complete, integrity-sealed model and a compatible dataset. Legacy packages must be retrained.</p></div>}
    </section>
    {result && <section className="panel" aria-label="Forecast result"><h3>{result.symbol} forecast · {result.horizon_bars} {result.interval} bar{result.horizon_bars === 1 ? '' : 's'} ahead</h3>
      <p>Data as of <strong>{new Date(result.as_of_ms).toISOString()}</strong>. Available at {new Date(result.available_at_ms).toISOString()}.</p>
      <div className="facts"><div><span>Last recorded close</span><strong>{number(result.last_close)}</strong></div><div><span>Predicted return</span><strong>{percent(result.predicted_return)}</strong></div><div><span>Implied close</span><strong>{number(result.implied_close)}</strong></div></div>
      {result.warnings.map(w => <p className="warning" key={w}>{w}</p>)}
      <details><summary>Forecast evidence</summary><pre>{JSON.stringify(result, null, 2)}</pre></details>
    </section>}
  </>;
}
