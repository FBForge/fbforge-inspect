import type { MappingField } from './types';
const full = (namespace: string, local: string) => `{${namespace}}${local}`;
export const ids = (text: string) => [...new Set(text.match(/\b[FGS]\d{5,}(?:V[\d.]+)?/gi) || [])];
export const bound = (depth: number, count: number) => {
  if (depth > 40 || count >= 2000)
    throw new Error('Schemastruktur überschreitet 40 Ebenen oder 2.000 Felder.');
};
export function addField(
  fields: MappingField[],
  names: string[],
  namespaces: string[],
  label: string,
  description: string,
  type: string,
  required: boolean,
  repeated: boolean,
  warnings: string[],
  attribute = false,
) {
  const key = names
    .map(
      (name, i) =>
        (attribute && i === names.length - 1 ? '@' : '') +
        full(encodeURIComponent(namespaces[i] || ''), encodeURIComponent(name)),
    )
    .join('/');
  if (fields.some((f) => f.key === key)) {
    warnings.push('Mehrere Schemazweige besitzen denselben Feldpfad. Auswahlregel prüfen.');
    const prior = fields.find((f) => f.key === key)!;
    prior.warnings = [...new Set([...prior.warnings, ...warnings])];
    return;
  }
  fields.push({
    key,
    path: names,
    names,
    namespaces,
    label: label || names.at(-1)!,
    description,
    identifiers: ids([...names, label, description].join(' ')),
    type,
    required,
    repeated,
    attribute,
    namespace: namespaces.at(-1) || '',
    warnings: [...new Set(warnings)],
  });
}
