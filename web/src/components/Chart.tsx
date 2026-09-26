import { useEffect, useRef, useState } from 'react';
import uPlot from 'uplot';
import 'uplot/dist/uPlot.min.css';
import type { Columns } from '../arrow/decode';
import { number, utc } from '../lib/format';

export function Chart({ title, data, x, ys, time = false }: { title: string; data: Columns; x: string; ys: string[]; time?: boolean }) {
  const container = useRef<HTMLDivElement>(null);
  const [table, setTable] = useState(false);
  useEffect(() => {
    if (!container.current || !data[x]?.length) return;
    const el = container.current;
    const palette = ['#2454cb', '#147d72', '#b83352'];
    const plot = new uPlot({ width: el.clientWidth || 600, height: 290,
      scales: { x: { time: false } },
      axes: [{ space: time ? 110 : 50, values: (_u, ticks) => ticks.map(t => time ? utc(t) : number(t, 0)) }, { size: 80 }],
      series: [{ label: time ? 'UTC date' : x }, ...ys.map((label, i) => ({ label, stroke: palette[i % palette.length], width: 2, points: { show: false } }))],
      legend: { show: true }, cursor: { drag: { x: true, y: false } },
    }, [data[x], ...ys.map(y => data[y] ?? [])], el);
    const observer = new ResizeObserver(() => plot.setSize({ width: el.clientWidth, height: 290 }));
    observer.observe(el);
    return () => { observer.disconnect(); plot.destroy(); };
  }, [data, x, ys.join(','), time]);
  return <section className="panel"><div className="panel-heading"><h2>{title}</h2><button onClick={() => setTable(!table)} aria-expanded={table}>{table ? 'Hide data table' : 'View data table'}</button></div>
    {data[x]?.length ? <div ref={container} role="img" aria-label={`${title}. ${data[x].length} observations. Data table available below.`} /> : <p>No observations in this range.</p>}
    {table && <DataTable data={data} columns={[x, ...ys]} />}
  </section>;
}

export function DataTable({ data, columns = Object.keys(data) }: { data: Columns; columns?: string[] }) {
  const [sort, setSort] = useState<{ name: string; desc: boolean }>({ name: columns[0], desc: false });
  const [top, setTop] = useState(0);
  const count = data[columns[0]]?.length ?? 0;
  const order = Array.from({ length: count }, (_, i) => i).sort((a, b) => ((data[sort.name]?.[a] ?? 0) - (data[sort.name]?.[b] ?? 0)) * (sort.desc ? -1 : 1));
  const start = Math.max(0, Math.floor(top / 36) - 4);
  const visible = order.slice(start, start + 30);
  return <><p className="muted">{count} observations returned. Large series may be sampled by the server. Select a column to sort.</p>
    <div className="table-scroll" tabIndex={0} aria-label="Scrollable data table" onScroll={e => setTop(e.currentTarget.scrollTop)}>
      <table aria-label="Series observations"><thead><tr>{columns.map(c => <th key={c} aria-sort={sort.name === c ? sort.desc ? 'descending' : 'ascending' : 'none'}><button onClick={() => setSort({ name: c, desc: sort.name === c ? !sort.desc : false })}>{c.replaceAll('_', ' ')}</button></th>)}</tr></thead>
        <tbody>{start > 0 && <tr aria-hidden="true"><td colSpan={columns.length} style={{ height: start * 36, padding: 0 }} /></tr>}
          {visible.map(i => <tr key={i}>{columns.map(c => <td key={c}>{c === 'timestamp_ms' ? utc(data[c][i]) : number(data[c]?.[i], 4)}</td>)}</tr>)}
          {count > start + 30 && <tr aria-hidden="true"><td colSpan={columns.length} style={{ height: (count - start - 30) * 36, padding: 0 }} /></tr>}
        </tbody></table>
    </div></>;
}
