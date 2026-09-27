import { tableFromIPC } from 'apache-arrow';
export type Columns = Record<string, number[]>;
export function decode(bytes: Uint8Array): Columns {
  const table = tableFromIPC(bytes);
  const columns: Columns = {};
  for (const field of table.schema.fields) {
    const vector = table.getChild(field.name);
    if (!vector) throw new Error(`Missing Arrow column ${field.name}`);
    columns[field.name] = Array.from(vector, (value: unknown) => {
      if (typeof value !== 'number' || !Number.isFinite(value)) throw new Error(`Invalid numeric value in ${field.name}`);
      return value;
    });
  }
  return columns;
}
export async function loadSeries(url: string): Promise<Columns> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Series unavailable (${response.status}). Regenerate the run and retry.`);
  return decodeResponse(response);
}

async function decodeResponse(response: Response): Promise<Columns> {
  const bytes = new Uint8Array(await response.arrayBuffer());
  // A transferable buffer keeps larger Arrow decoding off the UI thread.
  if (bytes.byteLength > 256_000) {
    const worker = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });
    return new Promise((resolve, reject) => {
      worker.onmessage = (event: MessageEvent<{ columns?: Columns; error?: string }>) => {
        worker.terminate();
        if (event.data.columns) resolve(event.data.columns); else reject(new Error(event.data.error));
      };
      worker.onerror = () => { worker.terminate(); reject(new Error('Arrow worker failed')); };
      worker.postMessage(bytes.buffer, [bytes.buffer]);
    });
  }
  return decode(bytes);
}

export async function loadPage(url: string): Promise<{ columns: Columns; total: number; offset: number; count: number }> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Table unavailable (${response.status}). Retry or inspect the artifact size.`);
  const readCount = (name: string) => {
    const value = response.headers.get(name);
    const count = Number(value);
    if (value === null || !Number.isSafeInteger(count) || count < 0) throw new Error('Missing or invalid pagination metadata');
    return count;
  };
  const total = readCount('x-total-count'), offset = readCount('x-offset'), count = readCount('x-returned-count');
  if (response.headers.get('x-sampled') !== 'false') throw new Error('Audit tables require complete, unsampled pages');
  return { columns: await decodeResponse(response), total, offset, count };
}
