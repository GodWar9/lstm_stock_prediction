export function number(value: number | undefined, digits = 2): string {
  return value === undefined || !Number.isFinite(value) ? 'Not recorded' : new Intl.NumberFormat('en-US', { maximumFractionDigits: digits, minimumFractionDigits: digits }).format(value);
}
export function percent(value: number | undefined): string {
  return value === undefined || !Number.isFinite(value) ? 'Not recorded' : `${number(value * 100)}%`;
}
export function utc(value: number): string { return new Date(value).toISOString().slice(0, 10); }
