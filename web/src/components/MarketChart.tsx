import { useState } from 'react';

export function MarketChart({ rows, label, currency }: { rows: number[][]; label: string; currency: string }) {
  const [mode, setMode] = useState('Area');
  const [hover, setHover] = useState<number>();
  const data = rows.slice(-100);
  const row = data[hover ?? data.length - 1];
  const low = Math.min(...data.map(r => r[1])), high = Math.max(...data.map(r => r[2]));
  const range = high - low || high * .01 || 1;
  const y = (p: number) => 30 + (high + range * .08 - p) / (range * 1.16) * 260;
  const step = 850 / Math.max(1, data.length);
  const maxVolume = Math.max(1, ...data.map(r => r[5]));
  const n = (v: number) => v.toLocaleString('en-US', { maximumFractionDigits: v < 1 ? 6 : 2 });
  return <div className="market-chart"><div className="chart-type" aria-label="Chart type">{['Area', 'Candles'].map(name => <button key={name} aria-pressed={mode === name} onClick={() => setMode(name)}>{name}</button>)}</div><div className="ohlc">{row ? <><time>{new Date(row[0] * 1000).toISOString().slice(0, 16).replace('T', ' ')} UTC</time>{['O', 'H', 'L', 'C'].map((key, i) => <span key={key}>{key} <b>{n(row[[3, 2, 1, 4][i]])}</b></span>)}</> : <span>Waiting for exchange candles</span>}</div>
    {data.length ? <svg viewBox="0 0 960 390" role="img" aria-label={`${label}. ${data.length} observations. Prices in ${currency}.`} onMouseLeave={() => setHover(undefined)} onMouseMove={e => { const bounds = e.currentTarget.getBoundingClientRect(); setHover(Math.max(0, Math.min(data.length - 1, Math.floor(((e.clientX - bounds.left) / bounds.width * 960 - 12) / step)))); }}>
      <defs><linearGradient id="market-area-fill" x1="0" y1="0" x2="0" y2="1"><stop offset="0%" stopColor="#ee8e37" stopOpacity=".24"/><stop offset="100%" stopColor="#ee8e37" stopOpacity="0"/></linearGradient></defs>
      {mode === 'Area' && <><path d={`M 12 300 ${data.map((r,i) => `L ${12+step*(i+.5)} ${y(r[4])}`).join(' ')} L ${12+step*(data.length-.5)} 300 Z`} fill="url(#market-area-fill)"/><polyline points={data.map((r,i)=>`${12+step*(i+.5)},${y(r[4])}`).join(' ')} fill="none" stroke="#e99650" strokeWidth="2"/></>}
      {Array.from({ length: 5 }, (_, i) => { const price = high - range * i / 4; return <g key={i}><line x1="12" x2="870" y1={y(price)} y2={y(price)} stroke="#232b38" strokeDasharray="3 5"/><text x="883" y={y(price) + 4} fill="#7e8b9f" fontSize="11">{n(price)}</text></g>; })}
      {data.map((r, i) => { const color = r[4] >= r[3] ? '#50c9a2' : '#ed7b87', x = 12 + step * (i + .5); return <g key={r[0]}>{mode === 'Candles' && <><line x1={x} x2={x} y1={y(r[2])} y2={y(r[1])} stroke={color}/><rect x={x - step * .3} y={Math.min(y(r[3]), y(r[4]))} width={Math.max(1, step * .6)} height={Math.max(1, Math.abs(y(r[3]) - y(r[4])))} fill={color}/></>}<rect x={x - step * .3} y={353 - r[5] / maxVolume * 40} width={Math.max(1, step * .6)} height={r[5] / maxVolume * 40} fill={color} opacity=".3"/>{i % Math.max(1, Math.floor(data.length / 5)) === 0 && <text x={x} y="378" textAnchor="middle" fontSize="11" fill="#7e8b9f">{new Date(r[0] * 1000).toISOString().slice(11, 16)}</text>}</g>; })}
      {hover !== undefined && <line x1={12 + step * (hover + .5)} x2={12 + step * (hover + .5)} y1="15" y2="355" stroke="#8d9bb1" strokeDasharray="4 4"/>}
    </svg> : <div className="chart-empty">No observations in this range.</div>}
    <details className="candle-table"><summary>View candle data</summary><div><table><thead><tr><th>UTC</th><th>Open</th><th>High</th><th>Low</th><th>Close</th><th>Volume</th></tr></thead><tbody>{data.slice().reverse().map(r => <tr key={r[0]}><td>{new Date(r[0] * 1000).toISOString().slice(0,16)}</td>{[r[3],r[2],r[1],r[4],r[5]].map((v,i)=><td key={i}>{n(v)}</td>)}</tr>)}</tbody></table></div></details>
  </div>;
}
