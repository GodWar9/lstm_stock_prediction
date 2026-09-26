import { describe, expect, it } from 'vitest';
import { number, percent, utc } from './format';
import { decode } from '../arrow/decode';
import { tableFromArrays, tableToIPC } from 'apache-arrow';
describe('rendering backend values', () => {
  it('distinguishes unavailable values from zero', () => {
    expect(number(undefined)).toBe('Not recorded');
    expect(number(NaN)).toBe('Not recorded');
    expect(percent(0)).toBe('0.00%');
    expect(percent(-0.125)).toBe('-12.50%');
  });
  it('uses UTC dates consistently', () => expect(utc(0)).toBe('1970-01-01'));
  it('preserves Arrow values and field order', () => {
    const ipc = tableToIPC(tableFromArrays({ timestamp_ms: new Float64Array([0, 1000]), nav: new Float64Array([100, 97.5]) }), 'file');
    expect(decode(ipc)).toEqual({ timestamp_ms: [0, 1000], nav: [100, 97.5] });
  });
  it('rejects unexpected non-numeric series', () => {
    const ipc = tableToIPC(tableFromArrays({ nav: ['not numeric'] }), 'file');
    expect(() => decode(ipc)).toThrow('Invalid numeric value');
  });
});
