import type { MappingField, MappingSchema, SchemaDocument } from './types';
import { children, first, qname, xmlDocument, XSD_NS } from './xml';

import { addField, bound } from './schemaFields';
import { jsonSchema, xdf } from './schemaFormats';
const full = (namespace: string, local: string) => `{${namespace}}${local}`;
const localName = (name: string) => name.replace(/^.*:/, '');
export function readSchema(
  documents: SchemaDocument[],
  rootName = '',
  mainName = '',
): MappingSchema {
  if (
    !documents.length ||
    documents.length > 32 ||
    documents.reduce((sum, d) => sum + d.text.length, 0) > 16 * 1024 * 1024
  )
    throw new Error('Bis zu 32 lokale Schemadateien mit zusammen höchstens 16 MiB auswählen.');
  if (new Set(documents.map((d) => d.name.toLocaleLowerCase('en'))).size !== documents.length)
    throw new Error('Schemadateinamen müssen innerhalb einer Seite eindeutig sein.');
  const main = documents.find((d) => d.name === mainName) || documents[0]!;
  if (main.text.trimStart().startsWith('<')) {
    const doc = xmlDocument(main.text);
    if (doc.documentElement.namespaceURI === XSD_NS && doc.documentElement.localName === 'schema')
      return xsd(documents, main, rootName);
    if (
      /xdatenfelder|xdf/i.test(doc.documentElement.namespaceURI || '') ||
      ['datenschema', 'stammdatenschema'].includes(doc.documentElement.localName)
    )
      return xdf(documents, main, doc);
    throw new Error('XSD oder XDatenfelder-Dokument erwartet.');
  }
  return jsonSchema(documents, main);
}

function xsd(documents: SchemaDocument[], main: SchemaDocument, selected: string): MappingSchema {
  const docs = documents.map((d) => ({ ...d, root: xmlDocument(d.text).documentElement }));
  if (docs.some((d) => d.root.namespaceURI !== XSD_NS || d.root.localName !== 'schema'))
    throw new Error('Ein XSD-Paket darf nur XSD-Dateien enthalten.');
  const owner = docs.find((d) => d.name === main.name)!,
    ns = owner.root.getAttribute('targetNamespace') || '';
  const warnings: string[] = [],
    fields: MappingField[] = [];
  const index = new Map<string, Element>();
  const parents = new Map<Element, typeof owner>();
  for (const doc of docs) {
    for (const e of Array.from(doc.root.getElementsByTagNameNS(XSD_NS, '*'))) parents.set(e, doc);
    for (const e of children(doc.root)) {
      const name = e.getAttribute('name');
      if (
        name &&
        ['element', 'attribute', 'complexType', 'simpleType', 'group', 'attributeGroup'].includes(
          e.localName,
        )
      ) {
        const key = e.localName + full(doc.root.getAttribute('targetNamespace') || '', name);
        if (index.has(key)) throw new Error(`Mehrdeutige XSD-Definition: ${name}.`);
        index.set(key, e);
      }
      if (['include', 'import', 'redefine'].includes(e.localName)) {
        const location = e.getAttribute('schemaLocation') || '';
        const file = location.replace(/\\/g, '/').split('/').at(-1)!;
        const dependency = docs.find((d) => d.name === file);
        if (location && !dependency) warnings.push(`Lokales Begleitschema fehlt: ${location}.`);
        if (
          dependency &&
          e.localName === 'import' &&
          (e.getAttribute('namespace') || '') !==
            (dependency.root.getAttribute('targetNamespace') || '')
        )
          warnings.push(`Namespace des Imports passt nicht zu ${file}.`);
        if (
          dependency &&
          e.localName === 'include' &&
          (dependency.root.getAttribute('targetNamespace') || '') !==
            (doc.root.getAttribute('targetNamespace') || '')
        )
          warnings.push(`Include mit abweichendem Namespace benötigt Prüfung: ${file}.`);
        if (e.localName === 'redefine')
          warnings.push('XSD redefine wird nicht vollständig aufgelöst.');
      }
    }
  }
  const roots = children(owner.root, 'element')
    .filter((e) => e.hasAttribute('name'))
    .map((e) => e.getAttribute('name')!);
  const rootName = selected || roots[0] || '';
  const root = children(owner.root, 'element').find((e) => e.getAttribute('name') === rootName);
  if (!root) throw new Error('Das XSD enthält kein ausgewähltes Wurzelelement.');
  const resolve = (e: Element, name: string, kind: string) => {
    const n = qname(e, name);
    return index.get(kind + full(n.namespace, n.local));
  };
  function element(
    e: Element,
    names: string[],
    namespaces: string[],
    required: boolean,
    repeated: boolean,
    notes: string[],
    stack: Element[],
    depth: number,
    global = false,
  ) {
    bound(depth, fields.length);
    const ref = e.getAttribute('ref');
    let item = e;
    if (ref) {
      const target = resolve(e, ref, e.localName);
      if (!target) {
        warnings.push(`Nicht aufgelöster Elementverweis: ${ref}.`);
        return;
      }
      item = target;
    }
    if (e.localName === 'attribute' && e.getAttribute('use') === 'prohibited') return;
    const attr = e.localName === 'attribute',
      doc = parents.get(item) || owner;
    const name = item.getAttribute('name');
    if (!name) return;
    if (/[\s/:{}]/.test(name)) warnings.push('XSD-Feldname benötigt Prüfung.');
    if (e.hasAttribute('abstract') || e.hasAttribute('substitutionGroup'))
      warnings.push('Abstrakte Elemente oder Ersetzungsgruppen benötigen Prüfung.');
    for (const attr of ['minOccurs', 'maxOccurs'])
      if (e.hasAttribute(attr) && !/^(?:[0-9]+|unbounded)$/.test(e.getAttribute(attr)!))
        warnings.push('XSD-Kardinalität ist nicht eindeutig lesbar.');
    const namespace =
      global ||
      ref ||
      item.getAttribute('form') === 'qualified' ||
      (!item.hasAttribute('form') &&
        doc.root.getAttribute(attr ? 'attributeFormDefault' : 'elementFormDefault') === 'qualified')
        ? doc.root.getAttribute('targetNamespace') || ''
        : '';
    const path = [...names, name],
      spaces = [...namespaces, namespace];
    const minimum = attr
      ? e.getAttribute('use') === 'required'
        ? 1
        : 0
      : Number(e.getAttribute('minOccurs') || 1);
    const many =
      repeated ||
      e.getAttribute('maxOccurs') === 'unbounded' ||
      Number(e.getAttribute('maxOccurs') || 1) > 1;
    const needed = required && minimum > 0;
    const description = Array.from(item.getElementsByTagNameNS(XSD_NS, 'documentation'))
      .map((x) => x.textContent?.trim() || '')
      .join(' ')
      .slice(0, 4000);
    let type = first(item, 'complexType') || first(item, 'simpleType');
    const typeName = item.getAttribute('type') || '';
    if (!type && typeName)
      type = resolve(item, typeName, 'complexType') || resolve(item, typeName, 'simpleType');
    if (type && stack.includes(type)) {
      warnings.push(`Rekursiver Typ bei ${path.join('/')}.`);
      return;
    }
    if (type && type.getElementsByTagNameNS(XSD_NS, 'simpleContent').length) {
      const base =
        type.getElementsByTagNameNS(XSD_NS, 'extension')[0]?.getAttribute('base') ||
        type.getElementsByTagNameNS(XSD_NS, 'restriction')[0]?.getAttribute('base') ||
        'unknown';
      addField(fields, path, spaces, name, description, localName(base), needed, many, notes, attr);
      structure(type, path, spaces, needed, many, notes, [...stack, type], depth + 1);
      return;
    }
    if (type && type.localName === 'complexType') {
      structure(type, path, spaces, needed, many, notes, [...stack, type], depth + 1);
      return;
    }
    const restriction = type?.getElementsByTagNameNS(XSD_NS, 'restriction')[0];
    const extension = type?.getElementsByTagNameNS(XSD_NS, 'extension')[0];
    const primitive = localName(
      restriction?.getAttribute('base') || extension?.getAttribute('base') || typeName || 'unknown',
    );
    const fieldNotes = [...notes];
    if (restriction && children(restriction).some((e) => e.localName !== 'annotation'))
      fieldNotes.push('XSD-Wertebeschränkungen müssen für die Zuordnung geprüft werden.');
    if (item.hasAttribute('fixed') || item.getAttribute('nillable') === 'true')
      fieldNotes.push(
        'Festwert oder erlaubter Leerwert benötigt eine eigene Zuordnungsentscheidung.',
      );
    if (!type && typeName && qname(item, typeName).namespace !== XSD_NS)
      fieldNotes.push(`Typ nicht aufgelöst: ${typeName}.`);
    addField(
      fields,
      path,
      spaces,
      description.split(/[\n.]/)[0] || name,
      description,
      primitive,
      needed,
      many,
      fieldNotes,
      attr,
    );
  }
  function structure(
    e: Element,
    names: string[],
    namespaces: string[],
    required: boolean,
    repeated: boolean,
    notes: string[],
    stack: Element[],
    depth: number,
  ) {
    bound(depth, fields.length);
    const nextNotes =
      e.localName === 'choice'
        ? [...notes, 'Feld liegt in einer XSD-Auswahl. Auswahlbedingung prüfen.']
        : notes;
    const needed = required && e.localName !== 'choice' && e.getAttribute('minOccurs') !== '0';
    const many =
      repeated ||
      e.getAttribute('maxOccurs') === 'unbounded' ||
      Number(e.getAttribute('maxOccurs') || 1) > 1;
    if (e.localName === 'extension' || e.localName === 'restriction') {
      const base = e.getAttribute('base');
      if (base) {
        const inherited = resolve(e, base, 'complexType');
        if (inherited && !stack.includes(inherited))
          structure(
            inherited,
            names,
            namespaces,
            needed,
            many,
            notes,
            [...stack, inherited],
            depth + 1,
          );
        else if (qname(e, base).namespace !== XSD_NS)
          warnings.push(`Basistyp nicht vollständig aufgelöst: ${base}.`);
      }
    }
    for (const child of children(e)) {
      if (child.namespaceURI !== XSD_NS) continue;
      if (['element', 'attribute'].includes(child.localName))
        element(child, names, namespaces, needed, many, nextNotes, stack, depth + 1);
      else if (['group', 'attributeGroup'].includes(child.localName) && child.hasAttribute('ref')) {
        const target = resolve(child, child.getAttribute('ref')!, child.localName);
        if (target && !stack.includes(target))
          structure(
            target,
            names,
            namespaces,
            needed,
            many,
            nextNotes,
            [...stack, target],
            depth + 1,
          );
        else warnings.push('XSD-Gruppe konnte nicht vollständig aufgelöst werden.');
      } else if (
        [
          'sequence',
          'all',
          'choice',
          'complexContent',
          'simpleContent',
          'extension',
          'restriction',
          'complexType',
        ].includes(child.localName)
      )
        structure(child, names, namespaces, needed, many, nextNotes, stack, depth + 1);
      else if (['any', 'anyAttribute'].includes(child.localName))
        warnings.push('Offener XSD-Inhalt benötigt eine eigene Zuordnungsentscheidung.');
    }
  }
  element(root, [], [], true, false, [], [], 0, true);
  if (!fields.length)
    throw new Error('Keine zuordenbaren Felder im ausgewählten XSD-Wurzelelement.');
  return {
    format: 'xsd',
    name: main.name,
    root: rootName,
    namespace: ns,
    documents,
    fields,
    warnings: [...new Set(warnings)],
    roots,
  };
}
