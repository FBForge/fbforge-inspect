export const arsPattern = '(?:DE)?(?:[0-9]{2}|[0-9]{3}|[0-9]{5}|[0-9]{9}|[0-9]{12})';

export function regionPattern(kind: string): string {
  if (kind === 'ars') return arsPattern;
  if (kind === 'ags') return '(?:DE)?[0-9]{8}';
  if (kind === 'areaId') return '[0-9]{1,20}';
  return '(?!)';
}

export function validRegion(kind: string, value: string): boolean {
  return new RegExp(`^(?:${regionPattern(kind)})$`).exec(value)?.[0] === value;
}
