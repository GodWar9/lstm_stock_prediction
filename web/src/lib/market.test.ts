import { describe, expect, it } from 'vitest';
import { candles, levels, numeric } from './market';
describe('public market validation', () => {
  it('orders candles chronologically and rejects impossible OHLC', () => {
    expect(candles([[120, 1, 3, 2, 3, 5], [60, 1, 3, 2, 2, 4]]).map(r => r[0])).toEqual([60, 120]);
    expect(() => candles([[60, 3, 2, 1, 1, 5]])).toThrow();
  });
  it('sorts depth and removes zero sizes', () => {
    expect(levels([['2', '3'], ['4', '0'], ['3', '1']], true)).toEqual([[3, 1], [2, 3]]);
    expect(levels([['2', '3'], ['1', '1']], false)[0]).toEqual([1, 1]);
  });
  it('rejects missing, negative and nonfinite values', () => {
    for (const v of [null, '', -1, 'NaN', Infinity, false]) expect(() => numeric(v)).toThrow();
  });
});
