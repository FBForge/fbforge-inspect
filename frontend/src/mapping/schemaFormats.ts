import type { MappingField, MappingSchema, SchemaDocument } from './types';
import { children, first, value, xmlDocument } from './xml';
import { addField, bound, ids } from './schemaFields';

export function jsonSchema(documents: SchemaDocument[], main: SchemaDocument): MappingSchema {
  let root: any;
  try {
    root = JSON.parse(main.text.replace(/^\uFEFF/, ''));
  } catch {
    throw new Error('JSON-Schema enthält einen Syntaxfehler.');
  }
  if (!root || typeof root !== 'object' || Array.isArray(root))
    throw new Error('JSON-Schemaobjekt erwartet.');
  const fields: MappingField[] = [],
    warnings: string[] = [];
  function walk(
    node: any,
    path: string[],
    required: boolean,
    repeated: boolean,
    stack: unknown[],
    depth: number,
  ) {
    bound(depth, fields.length);
    if (!node || typeof node !== 'object' || Array.isArray(node)) return;
    if (node.$ref) {
      if (typeof node.$ref !== 'string' || !node.$ref.startsWith('#/')) {
        warnings.push('Externe JSON-Referenz wird nicht automatisch geladen.');
        return;
      }
      let target = root;
      for (const segment of node.$ref.slice(2).split('/')) {
        const key = segment.replace(/~1/g, '/').replace(/~0/g, '~');
        target = target && Object.hasOwn(target, key) ? target[key] : undefined;
      }
      if (!target || stack.includes(target)) {
        warnings.push('JSON-Referenz fehlt oder ist rekursiv.');
        return;
      }
      walk(
        { ...target, ...node, $ref: undefined },
        path,
        required,
        repeated,
        [...stack, target],
        depth + 1,
      );
      return;
    }
    if (node.properties && typeof node.properties === 'object' && !Array.isArray(node.properties)) {
      for (const [key, child] of Object.entries(node.properties))
        walk(
          child,
          [...path, key],
          required && Array.isArray(node.required) && node.required.includes(key),
          repeated,
          stack,
          depth + 1,
        );
    } else if (node.items) walk(node.items, path, required, true, stack, depth + 1);
    else {
      const notes = ['oneOf', 'anyOf', 'allOf'].some((key) => Object.hasOwn(node, key))
        ? ['Zusammengesetztes JSON-Schema benötigt Prüfung.']
        : [];
      if (
        [
          'enum',
          'const',
          'pattern',
          'minimum',
          'maximum',
          'minLength',
          'maxLength',
          'multipleOf',
        ].some((key) => Object.hasOwn(node, key))
      )
        notes.push('JSON-Wertebeschränkungen müssen für die Zuordnung geprüft werden.');
      addField(
        fields,
        path.length ? path : ['$'],
        path.map(() => ''),
        typeof node.title === 'string' ? node.title : path.at(-1) || '$',
        typeof node.description === 'string' ? node.description : '',
        typeof node.format === 'string' && ['date', 'date-time', 'time'].includes(node.format)
          ? node.format.replace('date-time', 'dateTime')
          : typeof node.type === 'string'
            ? node.type
            : 'unknown',
        required,
        repeated,
        notes,
      );
    }
  }
  walk(root, [], true, false, [], 0);
  if (!fields.length) throw new Error('Keine zuordenbaren Felder im JSON-Schema.');
  return {
    format: 'json',
    name: main.name,
    root: '$',
    namespace: '',
    documents,
    fields,
    warnings: [...new Set(warnings)],
    roots: ['$'],
  };
}

export function xdf(
  documents: SchemaDocument[],
  main: SchemaDocument,
  doc: Document,
): MappingSchema {
  const fields: MappingField[] = [],
    warnings: string[] = [];
  const all = documents.flatMap((d) =>
    Array.from((d.name === main.name ? doc : xmlDocument(d.text)).getElementsByTagName('*')),
  );
  const schemas = Array.from(doc.getElementsByTagName('*')).filter((e) =>
    ['stammdatenschema', 'datenschema'].includes(e.localName),
  );
  const root = schemas[0] || doc.documentElement;
  const identification = (e: Element) => {
    const id = first(e, 'identifikation');
    return id
      ? value(id, 'id') + (value(id, 'version') ? 'V' + value(id, 'version') : '')
      : e.getAttribute('id') || '';
  };
  const definitions = new Map<string, Element>();
  for (const e of all.filter((e) => ['datenfeld', 'datenfeldgruppe'].includes(e.localName))) {
    const id = identification(e);
    if (id && (first(e, 'name') || first(e, 'struktur') || first(e, 'datentyp'))) {
      if (definitions.has(id)) throw new Error(`Mehrdeutige XDF-Definition: ${id}.`);
      definitions.set(id, e);
    }
  }
  function walk(
    e: Element,
    path: string[],
    labels: string[],
    required: boolean,
    repeated: boolean,
    stack: Element[],
    depth: number,
  ) {
    bound(depth, fields.length);
    const id = identification(e),
      label = value(e, 'bezeichnungEingabe') || value(e, 'name') || id;
    const context = [...path, id || label],
      textPath = [...labels, label];
    if (stack.includes(e)) {
      warnings.push('Rekursive XDF-Struktur benötigt Prüfung.');
      return;
    }
    const structures = children(e, 'struktur');
    if (structures.length) {
      for (const structure of structures) {
        const contains = first(structure, 'enthaelt');
        const direct =
          contains &&
          children(contains).find((c) => ['datenfeld', 'datenfeldgruppe'].includes(c.localName));
        const reference =
          contains && children(contains).find((c) => /verweis|referenz/i.test(c.localName));
        const target =
          direct && (first(direct, 'name') || first(direct, 'struktur'))
            ? direct
            : definitions.get(
                direct ? identification(direct) : reference ? identification(reference) : '',
              );
        if (!target) {
          warnings.push('XDF-Strukturverweis konnte nicht lokal aufgelöst werden.');
          continue;
        }
        const count = value(structure, 'anzahl') || value(structure, 'kardinalitaet') || '0:1';
        walk(
          target,
          context,
          textPath,
          required && !/^0/.test(count),
          repeated || /\*|n|unbounded/i.test(count) || Number(count.split(/[:,]/).at(-1)) > 1,
          [...stack, e],
          depth + 1,
        );
      }
    } else if (e.localName === 'datenfeld') {
      const datatype = first(e, 'datentyp');
      const type = datatype
        ? value(datatype, 'code') || datatype.textContent?.trim() || 'unknown'
        : 'unknown';
      const description = [
        ...textPath.slice(0, -1),
        value(e, 'beschreibung'),
        value(e, 'definition'),
      ]
        .filter(Boolean)
        .join(' · ');
      addField(
        fields,
        context,
        context.map(() => ''),
        label,
        description,
        type,
        required,
        repeated,
        [],
      );
      fields.at(-1)!.identifiers = ids(context.join(' '));
    }
  }
  if (children(root, 'struktur').length) walk(root, [], [], true, false, [], 0);
  else {
    warnings.push('XDF ohne aufgelöste Datenschemahierarchie. Feldkontexte müssen geprüft werden.');
    for (const e of definitions.values())
      if (e.localName === 'datenfeld') walk(e, [], [], false, false, [], 0);
  }
  if (!fields.length) throw new Error('Keine auflösbaren Datenfelder im XDF-Dokument.');
  if (all.some((e) => e.localName === 'regel' || e.localName === 'script'))
    warnings.push('XDF-Regeln und Skripte werden als Hinweise erhalten und nicht ausgeführt.');
  return {
    format: 'xdf',
    name: main.name,
    root: identification(root) || root.localName,
    namespace: root.namespaceURI || '',
    documents,
    fields,
    warnings: [...new Set(warnings)],
    roots: [identification(root) || root.localName],
  };
}
