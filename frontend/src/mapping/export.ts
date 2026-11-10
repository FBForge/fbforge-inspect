import type { MappingDecision, MappingField, MappingSchema, Requirements } from './types';
import { children, escapeXml as x, xmlDocument, XSD_NS } from './xml';
import { zipFiles } from './zip';
import { saveFile } from '../saveFile';

export function mappingReport(
  source: MappingSchema,
  target: MappingSchema,
  requirements: Requirements | null,
  decisions: MappingDecision[],
) {
  return {
    format: 'fbforge-inspect-mapping',
    formatVersion: 1,
    appVersion: '0.2.0',
    source: {
      name: source.name,
      format: source.format,
      root: source.root,
      fields: source.fields,
      warnings: source.warnings,
    },
    target: {
      name: target.name,
      format: target.format,
      root: target.root,
      fields: target.fields,
      warnings: target.warnings,
    },
    requirements,
    decisions,
    targetsWithoutProposal: target.fields
      .filter((field) => !decisions.some((d) => d.target === field.key && d.state !== 'skipped'))
      .map((field) => ({ key: field.key, path: field.path, required: field.required })),
    limitation:
      'Struktur- und Zuordnungshilfe. Keine Prüfung von Fachdaten und keine fachliche Freigabe.',
  };
}
export function mapforceLimit(source: MappingSchema, target: MappingSchema) {
  if (source.format !== 'xsd' || target.format !== 'xsd')
    return 'Für den MapForce-Entwurf werden Quell- und Ziel-XSD benötigt. XDF- und JSON-Zuordnungen können als Bericht gespeichert werden.';
  if (source.warnings.length || target.warnings.length)
    return 'Vor dem MapForce-Export die ungeklärten Schemaverweise und Strukturen ergänzen.';
  return '';
}
function component(
  schema: MappingSchema,
  role: 'source' | 'target',
  filename: string,
  keys: Map<string, number>,
  next: () => number,
) {
  const ns = [
    '',
    'http://www.altova.com/mapforce',
    ...new Set(schema.fields.flatMap((f) => f.namespaces).filter(Boolean)),
  ];
  interface Entry {
    name: string;
    namespace: string;
    key: number;
    children: Map<string, Entry>;
  }
  const tree: Entry = {
    name: schema.root,
    namespace: schema.namespace,
    key: next(),
    children: new Map(),
  };
  keys.set('root', tree.key);
  for (const field of schema.fields.filter((f) => !f.attribute)) {
    let node = tree;
    for (let i = 1; i < field.names.length; i++) {
      const key = `{${field.namespaces[i] || ''}}${field.names[i]}`;
      if (!node.children.has(key))
        node.children.set(key, {
          name: field.names[i]!,
          namespace: field.namespaces[i] || '',
          key: next(),
          children: new Map(),
        });
      node = node.children.get(key)!;
    }
    keys.set(field.key, node.key);
  }
  const port = role === 'source' ? 'outkey' : 'inpkey';
  const render = (e: Entry): string =>
    `<entry name="${x(e.name)}"${e.namespace ? ` ns="${ns.indexOf(e.namespace)}"` : ''} ${port}="${e.key}" expanded="1">${[...e.children.values()].map(render).join('')}</entry>`;
  return `<component name="${role === 'source' ? 'Quelle' : 'Ziel'}" library="xml" uid="${role === 'source' ? 2 : 3}" kind="14"><properties${role === 'target' ? ' XSLTDefaultOutput="1" XSLTTargetEncoding="UTF-8"' : ''}/><view ltx="${role === 'source' ? 20 : 700}" lty="40" rbx="${role === 'source' ? 330 : 1010}" rby="650"/><data><root><header><namespaces>${ns.map((n) => (n ? `<namespace uid="${x(n)}"/>` : '<namespace/>')).join('')}</namespaces></header><entry name="FileInstance" ns="1" expanded="1"><entry name="document" ns="1" expanded="1">${render(tree)}</entry></entry></root><document schema="${filename}" instanceroot="${x(`{${schema.namespace}}${schema.root}`)}"${role === 'target' ? ' outputinstance="ergebnis.xml"' : ''}/></data></component>`;
}
function schemaFiles(schema: MappingSchema, prefix: string) {
  const names = new Map(
    schema.documents.map((d, i) => [d.name, `${prefix}-${String(i + 1).padStart(2, '0')}.xsd`]),
  );
  const files = schema.documents.map((document) => {
    const doc = xmlDocument(document.text);
    for (const e of Array.from(doc.getElementsByTagNameNS(XSD_NS, '*'))) {
      if (
        !['include', 'import', 'redefine'].includes(e.localName) ||
        !e.hasAttribute('schemaLocation')
      )
        continue;
      const location = e.getAttribute('schemaLocation')!.replace(/\\/g, '/').split('/').at(-1)!;
      const replacement = names.get(location);
      if (!replacement)
        throw new Error(
          'Alle referenzierten XSD-Dateien müssen vor dem Export lokal geladen sein.',
        );
      e.setAttribute('schemaLocation', replacement);
    }
    // An export must not cause a schema processor to follow document-level base redirects.
    for (const e of [doc.documentElement, ...Array.from(doc.getElementsByTagName('*'))]) {
      e.removeAttributeNS('http://www.w3.org/XML/1998/namespace', 'base');
      for (const annotation of children(e, 'annotation')) annotation.remove();
    }
    const instructions = doc.createTreeWalker(doc, NodeFilter.SHOW_PROCESSING_INSTRUCTION);
    const remove: Node[] = [];
    for (let node = instructions.nextNode(); node; node = instructions.nextNode())
      remove.push(node);
    for (const node of remove) node.parentNode?.removeChild(node);
    return { name: names.get(document.name)!, content: new XMLSerializer().serializeToString(doc) };
  });
  return { files, main: names.get(schema.name)! };
}
export function mapforceFiles(
  source: MappingSchema,
  target: MappingSchema,
  requirements: Requirements | null,
  decisions: MappingDecision[],
) {
  const limit = mapforceLimit(source, target);
  if (limit) throw new Error(limit);
  const accepted = decisions.filter(
    (d) => ['automatic', 'confirmed'].includes(d.state) && d.target && !d.warnings.length,
  );
  const sources = new Map(source.fields.map((f) => [f.key, f])),
    targets = new Map(target.fields.map((f) => [f.key, f]));
  const direct = accepted.filter((d) => {
    const a = sources.get(d.source),
      b = targets.get(d.target);
    return a && b && !a.attribute && !b.attribute && !a.repeated && !b.repeated;
  });
  if (!direct.length)
    throw new Error(
      'Für den Entwurf wird mindestens eine geklärte direkte Zuordnung ohne Wiederholung oder Attribute benötigt.',
    );
  if (new Set(direct.map((d) => d.target)).size !== direct.length)
    throw new Error(
      'Mehrere Quellen teilen sich ein Ziel. Zuerst die Zusammenführungsregel klären.',
    );
  const src = schemaFiles(source, 'quelle'),
    dst = schemaFiles(target, 'ziel');
  let port = 1;
  const next = () => port++,
    left = new Map<string, number>(),
    right = new Map<string, number>();
  const leftComponent = component(source, 'source', src.main, left, next),
    rightComponent = component(target, 'target', dst.main, right, next);
  const connections = new Map<number, number[]>();
  connections.set(left.get('root')!, [right.get('root')!]);
  for (const d of direct)
    connections.set(left.get(d.source)!, [
      ...new Set([...(connections.get(left.get(d.source)!) || []), right.get(d.target)!]),
    ]);
  const graph = [...connections]
    .map(
      ([from, to]) =>
        `<vertex vertexkey="${from}"><edges>${to.map((key) => `<edge vertexkey="${key}"/>`).join('')}</edges></vertex>`,
    )
    .join('');
  const mapping = `<?xml version="1.0" encoding="UTF-8"?>\n<mapping xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="22"><resources/><component name="defaultmap" uid="1" editable="1" blackbox="0"><properties SelectedLanguage="builtin"/><structure><children>${leftComponent}${rightComponent}</children><graph directed="1"><edges/><vertices>${graph}</vertices></graph></structure></component></mapping>\n`;
  const report = {
    ...mappingReport(source, target, requirements, decisions),
    mapforce: {
      targetVersion: '2025',
      status: 'draft_requires_native_validation',
      exportedDirectMappings: direct.length,
      notExported: decisions
        .filter((d) => !direct.includes(d))
        .map((d) => ({
          source: d.source,
          target: d.target,
          state: d.state,
          reasons: d.warnings.length
            ? d.warnings
            : ['Nicht bestätigte oder noch nicht unterstützte Strukturregel.'],
        })),
    },
  };
  return [
    { name: 'zuordnung.mfd', content: mapping },
    ...src.files,
    ...dst.files,
    { name: 'zuordnung.json', content: JSON.stringify(report, null, 2) + '\n' },
    {
      name: 'README.txt',
      content: `FBForge Inspect 0.2.0 – MapForce-Entwurf für 2025\n\nzuordnung.mfd in MapForce öffnen. Die mitgelieferten XSD-Dateien im selben Ordner lassen. Die Quelleinstanz erst in MapForce auswählen.\n\n${direct.length} direkte Zuordnungen sind verbunden. ${decisions.length - direct.length} weitere Einträge stehen mit Begründung in zuordnung.json. Konstanten, Codelisten, Bedingungen, Attribute und Wiederholungen sind nicht automatisch als Transformationsregeln erzeugt.\n\nDer Export ist ein Entwurf. Das native Öffnen und die Ausführung müssen in Ihrer MapForce-2025-Installation geprüft werden. Eine fachliche Freigabe oder Instanzvalidierung wird nicht behauptet.\n`,
    },
  ];
}
export async function saveMappingArchive(files: { name: string; content: string }[]) {
  const bytes = zipFiles(files);
  let binary = '';
  for (let offset = 0; offset < bytes.length; offset += 16384)
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 16384));
  return saveFile(btoa(binary), 'zip');
}
export const displayPath = (field?: MappingField) =>
  field
    ? field.path
        .map((p, i) => (field.attribute && i === field.path.length - 1 ? '@' + p : p))
        .join(' / ')
    : 'Kein Ziel ausgewählt';
