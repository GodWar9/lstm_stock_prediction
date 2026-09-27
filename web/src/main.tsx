import React, { createContext, useContext, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { QueryClient, QueryClientProvider, useQuery, useQueryClient } from '@tanstack/react-query';
import { createRootRoute, createRoute, createRouter, RouterProvider, Link, Outlet, useNavigate } from '@tanstack/react-router';
import { json, object, numbers, type Run, type Model } from './api/client';
import { loadSeries } from './arrow/decode';
import { Chart, DataTable } from './components/Chart';
import { number, percent } from './lib/format';
import './style.css';

const client = new QueryClient({ defaultOptions: { queries: { retry: 1, staleTime: Infinity } } });
const RunContext = createContext<Run | undefined>(undefined);
const pages = [['/', 'Overview'], ['/backtest', 'Backtest'], ['/validation', 'Data and validation'], ['/models', 'Models'], ['/signals', 'Signals'], ['/risk', 'Risk and simulation'], ['/jobs', 'Run activity']] as const;
function Notice({ title, children, error = false }: { title: string; children?: React.ReactNode; error?: boolean }) {
  return <section className={`notice ${error ? 'error' : ''}`} role={error ? 'alert' : 'status'}><h2>{title}</h2>{children && <div>{children}</div>}</section>;
}
function Provenance({ run }: { run?: Run }) {
  const [copied, setCopied] = useState('');
  const values = run ? { 'Run ID': run.run_id, 'Data version': run.provenance.data_version, 'Model artifact': run.provenance.model_artifact_id, 'Git commit': run.provenance.git_commit, 'Config hash': run.provenance.config_hash } : {};
  return <footer className="provenance"><strong>Evidence trail</strong>{Object.entries(values).map(([label, value]) => <button title={value} key={label} onClick={() => { void navigator.clipboard.writeText(value).then(() => setCopied(label)).catch(() => setCopied('Copy unavailable; select the ID')); }}><span>{label}</span><code>{value}</code></button>)}<span aria-live="polite">{copied ? `${copied}${copied.includes('unavailable') ? '' : ' copied'}` : !run ? 'Select a run to inspect its provenance.' : ''}</span></footer>;
}
function Shell() {
  const search = rootRoute.useSearch();
  const navigate = useNavigate({ from: '/' });
  const queryClient = useQueryClient();
  const runs = useQuery({ queryKey: ['runs'], queryFn: () => json<Run[]>('/api/runs'), staleTime: 0 });
  const visible = runs.data?.filter(r => search.inSample || r.split === 'test') ?? [];
  const selected = visible.find(r => r.run_id === search.run) ?? visible[0];
  useEffect(() => {
    const events = new EventSource('/api/events');
    let previous = '';
    events.addEventListener('artifacts', event => {
      if (previous && previous !== event.data) void queryClient.invalidateQueries({ queryKey: ['runs'] });
      previous = event.data;
    });
    return () => events.close();
  }, [queryClient]);
  return <div className="app"><aside><Link className="brand" to="/" search={search}><span className="brand-mark">q</span><span>Quant<br /><small>Research inspector</small></span></Link><nav aria-label="Main navigation">{pages.map(([to, label]) => <Link key={to} to={to} search={search} activeProps={{ className: 'active' }} activeOptions={{ exact: true }}>{label}</Link>)}</nav><p className="sidebar-note">Local research workspace<br />Read-only artifact inspection</p></aside>
    <div className="workspace"><header><div><span className="muted">Inspect the evidence</span><h1>Research workspace</h1></div><div className="run-controls"><label htmlFor="run">Active run</label><select id="run" value={selected?.run_id ?? ''} onChange={e => void navigate({ search: { ...search, run: e.target.value } })}>{!visible.length && <option value="">No eligible runs</option>}{visible.map(r => <option key={r.run_id} value={r.run_id}>{r.kind} · {r.instruments.map(i => i.id).join(', ')} · {r.created_at.slice(0, 16)} · {r.run_id.slice(0, 8)}</option>)}</select><label className="checkbox"><input type="checkbox" checked={search.inSample} onChange={e => void navigate({ search: { run: undefined, inSample: e.target.checked } })} />Include in-sample and unverified runs</label></div></header>
      <main id="main"><RunContext.Provider value={selected}>
        {search.inSample && <div className="warning">In-sample and unverified results can overstate performance. Check the split and source before comparing runs.</div>}
        {selected && <div className="run-strip"><span className="badge">{selected.split === 'test' ? 'Held-out test' : selected.split}</span><strong>{selected.instruments.map(i => i.id).join(', ')}</strong><span>{selected.kind}</span><span>Source: {selected.provenance.source}</span><span>Model: {selected.provenance.model_artifact_id}</span></div>}
        {runs.isPending ? <Notice title="Reading local research artifacts">Loading manifests and provenance from the Rust API.</Notice> : runs.error ? <Notice title="Cannot load runs" error>{runs.error.message}<p>Start <code>quantctl serve --root .</code> from the project root, then retry.</p><button onClick={() => void runs.refetch()}>Retry</button></Notice> : <><Outlet />{selected?.warnings.map(w => <p className="method-note" key={w}>{w}</p>)}</>}
      </RunContext.Provider></main><Provenance run={selected} />
    </div></div>;
}
function NoRun() { return <Notice title="Your research starts with a recorded run"><p>Ingest data, build features, train a versioned model, then run a held-out backtest.</p><pre>quantctl --config configs/demo.yaml data ingest{'\n'}quantctl --config configs/demo.yaml features build{'\n'}quantctl --config configs/demo.yaml train{'\n'}quantctl --config configs/demo.yaml backtest run --model inspector_demo</pre><p>The demo configuration explicitly uses synthetic data.</p></Notice>; }
function PageTitle({ title, children }: { title: string; children: React.ReactNode }) { return <div className="page-title"><h2>{title}</h2><p>{children}</p></div>; }
const metrics = [
  ['sharpe', 'Sharpe ratio', 'Return relative to variability, annualized at 252 bars. Short samples and repeated selection can inflate it.', false],
  ['max_drawdown', 'Max drawdown', 'Largest decline from a previous portfolio peak. History cannot bound future losses.', true],
  ['total_return', 'Total return', 'Change in account value after simulated execution costs over this run.', true],
  ['turnover', 'Turnover', 'Absolute traded notional divided by initial capital. Higher values usually mean greater sensitivity to costs.', true],
] as const;
function Overview() {
  const run = useContext(RunContext);
  if (!run) return <NoRun />;
  return <><PageTitle title="A run, with its evidence">Inspect recorded results before forming a view about the model.</PageTitle><div className="metrics">{metrics.map(([key, label, help, pct]) => <article key={key}><div>{label}<details><summary aria-label={`About ${label}`}>?</summary><p>{help}</p></details></div><strong>{pct ? percent(run.metrics[key]) : number(run.metrics[key])}</strong></article>)}</div><SeriesPanel capability="equity_curve" file="equity.arrow" title="Portfolio value" x="timestamp_ms" ys={['nav']} time /><section className="panel"><h2>What this run contains</h2><div className="tags">{run.capabilities.map(c => <span key={c}>{c.replaceAll('_', ' ')}</span>)}</div><p>Run ID <code>{run.run_id}</code>. Created {new Date(run.created_at).toLocaleString()}.</p><p className="muted">Unavailable metrics are left unrecorded. No browser-side returns or risk estimates are added.</p></section></>;
}
function SeriesPanel({ capability, file, title, x, ys, time = false, params = '', table = false }: { capability: string; file: string; title: string; x: string; ys: string[]; time?: boolean; params?: string; table?: boolean }) {
  const run = useContext(RunContext);
  const query = useQuery({ queryKey: ['series', run?.run_id, file, params], enabled: !!run?.capabilities.includes(capability), queryFn: () => loadSeries(`/api/runs/${run!.run_id}/${file}?max_points=2000${params}`) });
  if (!run?.capabilities.includes(capability)) return <Notice title={`${title} is not recorded`}>This run does not include {capability.replaceAll('_', ' ')}. Generate the corresponding backtest or simulation artifact.</Notice>;
  if (query.isPending) return <Notice title={`Loading ${title.toLowerCase()}`}>Reading the backend Arrow series.</Notice>;
  if (query.error) return <Notice title={`Cannot load ${title.toLowerCase()}`} error>{query.error.message}<button onClick={() => void query.refetch()}>Retry</button></Notice>;
  if (table) return <section className="panel"><h2>{title}</h2><DataTable data={query.data} /></section>;
  return <Chart title={title} data={query.data} x={x} ys={ys} time={time} />;
}
function Backtest() {
  const run = useContext(RunContext);
  if (!run) return <NoRun />;
  return (
    <>
      <PageTitle title="Backtest inspection">
        Prior-bar signals drive next-bar execution. Read the simulated fills alongside the account history.
      </PageTitle>
      <SeriesPanel capability="equity_curve" file="equity.arrow" title="Equity curve · account currency" x="timestamp_ms" ys={['nav']} time />
      <SeriesPanel capability="benchmark" file="benchmark.arrow" title="Benchmark buy-and-hold · account currency" x="timestamp_ms" ys={['nav']} time />
      <SeriesPanel capability="drawdown" file="equity.arrow" title="Drawdown · fraction below peak" x="timestamp_ms" ys={['drawdown']} time />
      <SeriesPanel capability="positions" file="positions.arrow" title="Position history · shares" x="timestamp_ms" ys={['quantity']} time />
      <SeriesPanel capability="trades" file="trades.arrow" title="Trade blotter · signed shares and currency" x="timestamp_ms" ys={[]} table />
      {(!run.capabilities.includes('benchmark') || !run.capabilities.includes('positions')) && (
        <p className="method-note">Legacy runs do not record benchmark or position history. Retrain and rerun to produce complete evidence.</p>
      )}
    </>
  );
}
function Validation() {
  const run = useContext(RunContext);
  const q = useQuery({ queryKey: ['validation', run?.run_id], enabled: !!run?.capabilities.includes('validation'), queryFn: () => json<unknown>(`/api/runs/${run!.run_id}/validation`) });
  if (!run) return <NoRun />;
  const data = object(q.data);
  const folds = Array.isArray(data.folds) ? data.folds.map(object) : [];
  return <><PageTitle title="Data and validation">Follow the information boundary. Training, purge, embargo and held-out windows are recorded by the backend.</PageTitle>
    {!run.capabilities.includes('validation') ? <Notice title="Validation evidence is unavailable">Retrain the model and create a new backtest to record exact split boundaries.</Notice> : q.isPending ? <Notice title="Loading validation evidence" /> : q.error ? <Notice title="Validation could not be read" error>{q.error.message}</Notice> : <>
      <div className="facts"><div><span>Dataset</span><strong>{run.provenance.data_version}</strong></div><div><span>PIT label boundaries</span><strong>{data.pit_passed === true ? 'Passed' : 'Not verified'}</strong></div><div><span>Scaler fitting</span><strong>{data.scaler_train_only === true ? 'Training rows only' : 'Not verified'}</strong></div></div>
      <section className="panel timeline-panel"><h2>Chronological split timeline</h2><p>{String(data.evaluation ?? 'Recorded training folds')}</p>{folds.map((fold, index) => {
        const segments = Array.isArray(fold.segments) ? fold.segments.map(object) : [];
        return <div className="fold" key={index}><strong>Fold {String(fold.fold ?? index + 1)}</strong><div className="timeline">{segments.map((s, i) => <div key={i} className={`segment ${String(s.kind)}`} style={{ flexGrow: Math.max(1, Number(s.end_row) - Number(s.start_row)) }} title={`${s.kind}: rows ${s.start_row} to ${s.end_row} (exclusive)`}><span>{String(s.kind)}</span><small>{String(s.start_row)}–{String(s.end_row)}</small></div>)}</div><table><thead><tr><th>Window</th><th>First row</th><th>Exclusive end</th><th>Purpose</th></tr></thead><tbody>{segments.map((s, i) => <tr key={i}><td>{String(s.kind)}</td><td>{String(s.start_row)}</td><td>{String(s.end_row)}</td><td>{s.kind === 'purge' ? 'Remove labels that could overlap later windows.' : s.kind === 'embargo' ? 'Separate windows to reduce temporal dependence.' : s.kind === 'train' ? 'Fit model weights and scaler statistics.' : s.kind === 'test' ? 'Evaluate unseen observations; repeated use can bias selection.' : 'Choose training checkpoints.'}</td></tr>)}</tbody></table></div>;
      })}</section><details className="panel"><summary>Exact validation record</summary><pre>{JSON.stringify(q.data, null, 2)}</pre></details></>}
  </>;
}
function ModelCard({ model }: { model: Model }) {
  const meta = object(model.metadata), history = object(model.training_log), validation = object(model.validation);
  const train = numbers(history.train_loss), val = numbers(history.val_loss);
  const parity = object(validation.onnx_parity);
  return <article className="model-card"><h3>{model.artifact_id}</h3><p>{model.onnx_present ? 'ONNX artifact available' : 'ONNX artifact missing'}</p><dl><dt>Architecture</dt><dd>{JSON.stringify(meta.architecture ?? 'Not recorded')}</dd><dt>Training data</dt><dd>{String(meta.training_dataset_version ?? 'Not recorded')}</dd><dt>Numerical parity</dt><dd>{parity.passed === true ? `Passed · max error ${String(parity.max_abs_error)}` : 'Not recorded'}</dd><dt>Evaluation metrics</dt><dd><pre>{JSON.stringify(meta.evaluation_metrics ?? {}, null, 2)}</pre></dd></dl>
    {train.length > 0 && <Chart title="Training loss" data={{ epoch: train.map((_, i) => i + 1), train_loss: train }} x="epoch" ys={['train_loss']} />}
    {val.length > 0 && <Chart title="Validation loss" data={{ epoch: val.map((_, i) => i + 1), validation_loss: val }} x="epoch" ys={['validation_loss']} />}
  </article>;
}
function Models() {
  const q = useQuery({ queryKey: ['models'], queryFn: () => json<Model[]>('/api/models'), staleTime: 0 });
  const [left, setLeft] = useState(''), [right, setRight] = useState('');
  const models = q.data ?? [];
  const first = models.find(m => m.artifact_id === left) ?? models[0];
  const second = models.find(m => m.artifact_id === right);
  return <><PageTitle title="Model artifacts">Compare the recorded training evidence. A parity check verifies numerical export, not predictive usefulness.</PageTitle>{q.isPending ? <Notice title="Loading model registry" /> : q.error ? <Notice title="Cannot load models" error>{q.error.message}</Notice> : !models.length ? <Notice title="No trained models">Run quantctl train after generating an Arrow training dataset.</Notice> : <><div className="compare-select">{[['Primary artifact', first?.artifact_id ?? '', setLeft], ['Compare with', right, setRight]].map(([label, value, set]) => <label key={String(label)}>{String(label)}<select value={String(value)} onChange={e => (set as React.Dispatch<React.SetStateAction<string>>)(e.target.value)}><option value="">Choose an artifact</option>{models.map(m => <option key={m.artifact_id}>{m.artifact_id}</option>)}</select></label>)}</div><div className={`model-grid ${second ? 'compare' : ''}`}>{first && <ModelCard model={first} />}{second && <ModelCard model={second} />}</div></>}</>;
}
function Signals() {
  const run = useContext(RunContext);
  const [asof, setAsof] = useState('');
  if (!run) return <NoRun />;
  const timestamp = asof ? Date.parse(`${asof}T23:59:59Z`) : Date.now();
  const params = `&asof=${timestamp}&instrument=${encodeURIComponent(run.instruments[0]?.id ?? '')}`;
  return (
    <>
      <PageTitle title="Signal inspection">
        Predictions available as of the selected UTC date. Expected returns are model outputs, not realized outcomes.
      </PageTitle>
      <label className="asof">As of (UTC)<input type="date" value={asof} onChange={e => setAsof(e.target.value)} /></label>
      <SeriesPanel capability="signals" file="signals.arrow" title="Expected forward log return" x="timestamp_ms" ys={['expected_return']} time params={params} />
      <SeriesPanel capability="signals" file="signals.arrow" title="Signal confidence · fraction" x="timestamp_ms" ys={['confidence']} time params={params} />
      <SeriesPanel capability="signals" file="signals.arrow" title="Signal observations" x="timestamp_ms" ys={[]} table params={params} />
      <SeriesPanel capability="signal_outcomes" file="signal_outcomes.arrow" title="Signal outcomes · expected vs realized return" x="timestamp_ms" ys={['expected_return', 'realized_return']} time params={params} />
      <SeriesPanel capability="signal_outcomes" file="signal_outcomes.arrow" title="Signal outcome table" x="timestamp_ms" ys={[]} table params={params} />
      {!run.capabilities.includes('signal_outcomes') && (
        <p className="method-note">Realized targets, calibration and rolling IC are not exported yet for legacy runs. Confidence is a model signal score, not a calibrated probability.</p>
      )}
    </>
  );
}
function Risk() {
  const run = useContext(RunContext);
  const q = useQuery({ queryKey: ['simulation', run?.run_id], enabled: !!run?.capabilities.includes('monte_carlo'), queryFn: () => json<unknown>(`/api/runs/${run!.run_id}/simulation.json`) });
  const riskQuery = useQuery({ queryKey: ['risk', run?.run_id], enabled: !!run?.capabilities.includes('risk'), queryFn: () => json<unknown>(`/api/runs/${run!.run_id}/risk.json`) });
  if (!run) return <NoRun />;
  const result = object(q.data);
  const riskData = object(riskQuery.data);
  const stressScenarios = Array.isArray(riskData.stress_scenarios) ? riskData.stress_scenarios.map(object) : [];
  const factorExposure = object(riskData.factor_exposure);

  return (
    <>
      <PageTitle title="Risk and simulation">Inspect a range of resampled outcomes and point-in-time parametric risk metrics.</PageTitle>

      {riskQuery.error && <Notice title="Risk report could not be loaded" error>{String(riskQuery.error)}</Notice>}
      {run.capabilities.includes('risk') && (
        <section className="panel">
          <h2>Point-in-time risk evaluation</h2>
          <div className="facts">
            <div><span>VaR (95% 1-day)</span><strong>{typeof riskData.var_95 === 'number' ? number(riskData.var_95) : 'Not recorded'}</strong></div>
            <div><span>CVaR (95% 1-day)</span><strong>{typeof riskData.cvar_95 === 'number' ? number(riskData.cvar_95) : 'Not recorded'}</strong></div>
            <div><span>Realized Volatility (ann.)</span><strong>{typeof riskData.volatility === 'number' ? percent(riskData.volatility) : 'Not recorded'}</strong></div>
            <div><span>Market Beta</span><strong>{typeof riskData.beta === 'number' ? number(riskData.beta) : 'Not recorded'}</strong></div>
            <div><span>Gross Exposure</span><strong>{typeof riskData.gross_exposure === 'number' ? percent(riskData.gross_exposure) : 'Not recorded'}</strong></div>
            <div><span>Net Exposure</span><strong>{typeof riskData.net_exposure === 'number' ? percent(riskData.net_exposure) : 'Not recorded'}</strong></div>
          </div>
          {stressScenarios.length > 0 && (
            <>
              <h3>Stress testing scenarios</h3>
              <table>
                <thead><tr><th>Scenario</th><th>Estimated PnL</th><th>Estimated Return</th></tr></thead>
                <tbody>
                  {stressScenarios.map((s, i) => (
                    <tr key={i}>
                      <td>{String(s.scenario_name)}</td>
                      <td>{typeof s.estimated_pnl === 'number' ? number(s.estimated_pnl) : 'N/A'}</td>
                      <td>{typeof s.estimated_pnl_pct === 'number' ? percent(s.estimated_pnl_pct) : 'N/A'}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </>
          )}
          {Object.keys(factorExposure).length > 0 && (
            <>
              <h3>Factor exposures</h3>
              <table>
                <thead><tr><th>Factor</th><th>Exposure</th></tr></thead>
                <tbody>
                  {Object.entries(factorExposure).map(([k, v]) => (
                    <tr key={k}><td>{k}</td><td>{typeof v === 'number' ? number(v) : String(v)}</td></tr>
                  ))}
                </tbody>
              </table>
            </>
          )}
        </section>
      )}

      {!run.capabilities.includes('risk') && (
        <Notice title="Point-in-time risk evaluation is not recorded">
          Legacy runs do not include point-in-time parametric risk reports. Re-run backtest to generate risk metrics.
        </Notice>
      )}

      <SeriesPanel capability="monte_carlo" file="simulation/bands.arrow" title="Portfolio scenarios · p5 / median / p95" x="step" ys={['p5', 'p50', 'p95']} />
      {!run.capabilities.includes('monte_carlo') && <p className="method-note">Run <code>quantctl simulate --report reports/runs/{run.run_id}/report.json --paths 1000</code>, then include unverified runs and select the new simulation.</p>}
      {q.error && <Notice title="Simulation summary unavailable" error>{q.error.message}</Notice>}
      {q.data !== undefined && (
        <section className="panel">
          <h2>Outcome distributions</h2>
          <p>{String(result.num_paths)} paths. Probability of 50% drawdown: {typeof result.prob_of_ruin === 'number' ? percent(result.prob_of_ruin) : 'Not recorded'}.</p>
          <table>
            <thead><tr><th>Metric</th><th>5th percentile</th><th>Median</th><th>95th percentile</th></tr></thead>
            <tbody>
              {['sharpe_distribution', 'drawdown_distribution', 'cagr_distribution'].map(key => {
                const d = object(result[key]);
                return (
                  <tr key={key}>
                    <td>{key.replaceAll('_', ' ')}</td>
                    {['p5', 'p50', 'p95'].map(p => (
                      <td key={p}>{typeof d[p] === 'number' ? key === 'sharpe_distribution' ? number(d[p]) : percent(d[p]) : 'Not recorded'}</td>
                    ))}
                  </tr>
                );
              })}
            </tbody>
          </table>
        </section>
      )}
    </>
  );
}
function Jobs() {
  const q = useQuery({ queryKey: ['runs'], queryFn: () => json<Run[]>('/api/runs'), staleTime: 0 });
  return <><PageTitle title="Run activity">Published artifacts appear automatically through the local event stream.</PageTitle><section className="panel"><h2>Completed runs</h2><p>No job execution is triggered from this read-only inspector. In-progress percentages are not recorded by the CLI.</p><table><thead><tr><th>Run</th><th>Kind</th><th>Created</th><th>Status</th></tr></thead><tbody>{q.data?.map(r => <tr key={r.run_id}><td><Link to="/" search={{ run: r.run_id, inSample: r.split !== 'test' }}>{r.run_id.slice(0, 12)}</Link></td><td>{r.kind}</td><td>{new Date(r.created_at).toLocaleString()}</td><td>Published</td></tr>)}</tbody></table></section></>;
}
const rootRoute = createRootRoute({ component: Shell, validateSearch: (search: Record<string, unknown>) => ({ run: typeof search.run === 'string' ? search.run : undefined, inSample: search.inSample === true || search.inSample === 'true' }) });
const routeTree = rootRoute.addChildren([
  createRoute({ getParentRoute: () => rootRoute, path: '/', component: Overview }),
  createRoute({ getParentRoute: () => rootRoute, path: '/backtest', component: Backtest }),
  createRoute({ getParentRoute: () => rootRoute, path: '/validation', component: Validation }),
  createRoute({ getParentRoute: () => rootRoute, path: '/models', component: Models }),
  createRoute({ getParentRoute: () => rootRoute, path: '/signals', component: Signals }),
  createRoute({ getParentRoute: () => rootRoute, path: '/risk', component: Risk }),
  createRoute({ getParentRoute: () => rootRoute, path: '/jobs', component: Jobs }),
]);
const router = createRouter({ routeTree });
declare module '@tanstack/react-router' { interface Register { router: typeof router } }
createRoot(document.getElementById('root')!).render(<React.StrictMode><QueryClientProvider client={client}><RouterProvider router={router} /></QueryClientProvider></React.StrictMode>);
