import type {
  CodeValue,
  OnlineService,
  OnlineParameter,
  ContextRow,
  ReportData,
} from './reportData';

/** Responses can omit arrays; presentation never treats a scalar as a record list. */
export function records<T extends object = Record<string, unknown>>(value: unknown): T[] {
  return Array.isArray(value)
    ? value.filter((item): item is T => !!item && typeof item === 'object' && !Array.isArray(item))
    : [];
}
export function list(value: unknown): string {
  return Array.isArray(value) ? value.map(String).join(', ') : '';
}
export function identifier(value: unknown): string {
  if (typeof value === 'string') return value;
  if (value && typeof value === 'object') {
    const id = value as Record<string, unknown>;
    return [id.schemeAgencyID, id.schemeID, id.schemeVersionID, id.value]
      .filter((part) => typeof part === 'string' && part)
      .join(' · ');
  }
  return 'Keine ID veröffentlicht';
}
export function codeLabel(value: CodeValue): string {
  return [value.code, value.name].filter((part) => typeof part === 'string' && part).join(' · ');
}
export function onlineName(value: OnlineService): string {
  const names = records<{ languageCode?: string; value?: string }>(value.bezeichnung);
  return (names.find((name) => name.languageCode === 'de') || names[0])?.value || '';
}
export function parameters(service: OnlineService) {
  return [
    ...records<OnlineParameter>(service.parameterEfA).map((parameter, index) => ({
      ...parameter,
      kind: 'EfA',
      name: codeLabel(parameter.parameterEfA || {}),
      path: `parameterEfA[${index}]`,
    })),
    ...records<OnlineParameter>(service.parameterIndividuell).map((parameter, index) => ({
      ...parameter,
      kind: 'Individuell',
      name: parameter.parameterName,
      path: `parameterIndividuell[${index}]`,
    })),
  ];
}
export function limitValue(value: unknown, field: string): string {
  if (typeof value !== 'number') return 'Keine Angabe';
  const number = new Intl.NumberFormat('de-DE').format(value);
  return field.endsWith('Bytes')
    ? `${new Intl.NumberFormat('de-DE', { maximumFractionDigits: 2 }).format(value / 1048576)} MiB (${number} Byte)`
    : number;
}
export function canOpen(value: unknown): value is string {
  return typeof value === 'string' && /^https:\/\//i.test(value);
}

export function routingRows(data: ReportData): ContextRow[] {
  return records<ContextRow>(data.routes || data.destinations || data.areas || []);
}
