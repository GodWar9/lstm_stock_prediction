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
  const marketStatus = useQuery({ queryKey: ['market-status'], queryFn: () => json<components['schemas']['MarketStatus']>('/api/market/status'), staleTime: 60_000 });
  const [fresh, setFresh] = useState(false);
  const [modelId, setModelId] = useState('');
  const [datasetKey, setDatasetKey] = useState('');
  const selected = datasets.data?.find(d => `${d.dataset}/${d.symbol}` === datasetKey);
  const model = models.data?.find(m => m.artifact_id === modelId);
  const interval = model ? String(object(model.metadata).bar_interval ?? '1d') : undefined;
  const metadata = object(model?.metadata);
  const modelMarket = object(metadata.market);
  const datasetMarket = object(selected?.market);
  const identityFields = ['provider_symbol', 'exchange', 'currency', 'timezone', 'instrument_type', 'periods_per_year'];
  const marketMismatch = !!model && !!selected && identityFields.some(key => modelMarket[key] !== datasetMarket[key]);
  const freshCompatible = metadata.source === 'twelve_data:unadjusted:rest_snapshot'
    && selected?.source === metadata.source && metadata.training_symbol === selected?.symbol
    && !!modelMarket.provider_symbol && !marketMismatch;
  const mismatch = !!selected && !!interval && selected.interval !== interval;
  const forecast = useMutation({ mutationFn: (query: URLSearchParams) => json<Result>(`/api/forecast?${query}`), retry: false });
  const reset = () => forecast.reset();
  const result = forecast.data;
  return <>
    <div className="page-title"><h2>Make a forecast</h2><p>Forecast from a recorded dataset or fetch the latest completed bars for a matching global market model.</p></div>
    <div className="warning">Research forecasts. Check the source, exchange, currency and data timestamp. Recent bars do not prove a real-time feed or predictive accuracy.</div>
    <section className="panel"><h3>Forecast inputs</h3>
      <label>Data mode<select aria-label="Forecast data mode" value={fresh ? 'fresh' : 'recorded'} disabled={forecast.isPending} onChange={e => { setFresh(e.target.value === 'fresh'); reset(); }}><option value="recorded">Recorded dataset</option><option value="fresh">Fresh global market bars</option></select></label>
      {fresh && <div className="warning">
        <p>{marketStatus.data?.message ?? (marketStatus.error ? 'Unable to check market data setup.' : 'Checking market data setup…')}</p>
        {marketStatus.error && <button onClick={() => { void marketStatus.refetch(); }}>Retry setup check</button>}
        <p>Fresh mode uses the selected model’s exchange and currency. Completed minute bars must be no more than three minutes old; daily bars may be up to four days old. Provider feed delay is unverified.</p>
        {model && selected && !freshCompatible && <p>Train a model on matching Twelve Data history to enable fresh forecasts. Demo and legacy models support recorded mode.</p>}
      </div>}
      {(models.isPending || datasets.isPending) && <p role="status">The backend may take a minute to wake up. Loading models and datasets…</p>}
      {(models.error || datasets.error) && <div role="alert">{models.error?.message ?? datasets.error?.message}<button onClick={() => { void models.refetch(); void datasets.refetch(); }}>Reload inputs</button></div>}
      {models.data?.length === 0 && <p>No trained models are available. Train a model with <code>quantctl train</code> before forecasting.</p>}
      {datasets.data?.length === 0 && <p>No recorded datasets are available. Ingest market data with <code>quantctl data ingest</code> or import a completed Alpaca capture with <code>quantctl research --journal …</code>.</p>}
      <div className="forecast-inputs"><label>Model<select aria-label="Forecast model" value={modelId} disabled={forecast.isPending} onChange={e => { setModelId(e.target.value); reset(); }}><option value="">Select a model</option>{models.data?.map(m => <option key={m.artifact_id} value={m.artifact_id} disabled={!m.onnx_present}>{m.artifact_id}{!m.onnx_present ? ' (ONNX missing)' : ''}</option>)}</select></label>
      <label>Dataset and symbol<select aria-label="Forecast dataset" value={datasetKey} disabled={forecast.isPending} onChange={e => { setDatasetKey(e.target.value); reset(); }}><option value="">Select a dataset</option>{datasets.data?.map(d => <option key={`${d.dataset}/${d.symbol}`} value={`${d.dataset}/${d.symbol}`}>{d.symbol} · {d.interval} · {d.dataset} · {d.source}</option>)}</select></label></div>
      {selected && <p>Source: {selected.source}. {selected.bars.toLocaleString()} recorded {selected.interval} bars.</p>}
      {!!selected?.market && <p>Market: {String(datasetMarket.provider_symbol)} · {String(datasetMarket.exchange)} · {String(datasetMarket.currency)} · {String(datasetMarket.timezone)}</p>}
      {marketMismatch && <p role="alert">Model and dataset market identities differ. Select the same symbol, exchange, currency and feature settings.</p>}
      {mismatch && <p role="alert">Model uses {interval} bars; this dataset uses {selected?.interval}. Select matching intervals.</p>}
      <button disabled={!model || !selected || mismatch || marketMismatch || forecast.isPending || (fresh && (!marketStatus.data?.configured || !freshCompatible))} onClick={() => { if (selected) forecast.mutate(new URLSearchParams({ model: modelId, dataset: selected.dataset, symbol: selected.symbol, fresh: String(fresh) })); }}>{forecast.isPending ? 'Computing forecast…' : 'Generate forecast'}</button>
      {forecast.isPending && <p role="status">Verifying artifact hashes, building features and running inference…</p>}
      {forecast.error && <div role="alert"><h3>Forecast unavailable</h3><p>{forecast.error.message}</p><p>Use a complete, integrity-sealed model and a compatible dataset. Legacy packages must be retrained.</p></div>}
    </section>
    {result && <section className="panel" aria-label="Forecast result"><h3>{result.symbol} forecast · {result.horizon_bars} {result.interval} bar{result.horizon_bars === 1 ? '' : 's'} ahead</h3>
      <p>{result.input_mode === 'fresh' ? 'Fresh market snapshot' : 'Recorded dataset'} · Currency: {String(object(result.market).currency ?? 'not specified')} · Feed delay: {result.feed_latency ?? 'unknown'}</p>
      {result.fetched_at_ms != null && <p>Fetched at {new Date(result.fetched_at_ms).toISOString()}. Data age at prediction: {Math.round(result.data_age_ms / 1000)} seconds.</p>}
      <p>Data as of <strong>{new Date(result.as_of_ms).toISOString()}</strong>. Available at {new Date(result.available_at_ms).toISOString()}.</p>
      <div className="facts"><div><span>Last recorded close</span><strong>{number(result.last_close)}</strong></div><div><span>Predicted return</span><strong>{percent(result.predicted_return)}</strong></div><div><span>Implied close</span><strong>{number(result.implied_close)}</strong></div></div>
      {result.warnings.map(w => <p className="warning" key={w}>{w}</p>)}
      <details><summary>Forecast evidence</summary><pre>{JSON.stringify(result, null, 2)}</pre></details>
    </section>}
  </>;
}
