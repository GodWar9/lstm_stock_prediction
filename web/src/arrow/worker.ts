import { decode } from './decode';
self.onmessage = (event: MessageEvent<ArrayBuffer>) => {
  try { self.postMessage({ columns: decode(new Uint8Array(event.data)) }); }
  catch (error) { self.postMessage({ error: String(error) }); }
};
