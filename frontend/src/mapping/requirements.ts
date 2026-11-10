import { children, first, value, xmlDocument } from './xml';
import { readZip } from './zip';
import type { Requirement, RequirementColumns, Requirements, WorkbookSheet } from './types';

const column = (address: string) => address.replace(/[0-9]/g, '');
const clean = (text: string) =>
  text
    .toLocaleLowerCase('de')
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '');
export async function readWorkbook(bytes: Uint8Array): Promise<WorkbookSheet[]> {
  const files = await readZip(bytes);
  if (!files.has('xl/workbook.xml') || !files.has('[Content_Types].xml'))
    throw new Error('XLSX-Arbeitsmappe erwartet.');
  const xml = (path: string) => {
    const text = files.get(path);
    if (!text) throw new Error(`XLSX-Bestandteil fehlt: ${path}`);
    return xmlDocument(text);
  };
  const contentTypes = xml('[Content_Types].xml');
  if (/macroEnabled|vbaProject/i.test(new XMLSerializer().serializeToString(contentTypes)))
    throw new Error(
      'Makro-Arbeitsmappen werden nicht unterstützt. Bitte als XLSX ohne Makros speichern.',
    );
  const workbook = xml('xl/workbook.xml');
  const relationships = xml('xl/_rels/workbook.xml.rels');
  const targets = new Map<string, string>();
  for (const entry of Array.from(relationships.documentElement.children)) {
    if (!/\/worksheet$/.test(entry.getAttribute('Type') || '')) continue;
    if (entry.getAttribute('TargetMode') === 'External') continue;
    const target = entry.getAttribute('Target') || '';
    const path = target.startsWith('/') ? target.slice(1) : 'xl/' + target;
    if (path.includes('..') || path.includes('\\') || !path.startsWith('xl/'))
      throw new Error('Ungültiger interner XLSX-Verweis.');
    targets.set(entry.getAttribute('Id') || '', path);
  }
  const strings = files.has('xl/sharedStrings.xml')
    ? Array.from(xml('xl/sharedStrings.xml').getElementsByTagNameNS('*', 'si')).map((entry) =>
        Array.from(entry.getElementsByTagNameNS('*', 't'))
          .map((t) => t.textContent || '')
          .join(''),
      )
    : [];
  const sheets = Array.from(workbook.getElementsByTagNameNS('*', 'sheet'));
  if (sheets.length > 32) throw new Error('Höchstens 32 Tabellenblätter werden unterstützt.');
  let cellCount = 0;
  return sheets.map((sheet) => {
    const relation =
      sheet.getAttributeNS(
        'http://schemas.openxmlformats.org/officeDocument/2006/relationships',
        'id',
      ) || '';
    const path = targets.get(relation);
    if (!path) throw new Error('Tabellenblatt verweist nicht auf einen lokalen XLSX-Inhalt.');
    const doc = xml(path),
      rows = Array.from(doc.getElementsByTagNameNS('*', 'row'));
    if (rows.length > 20000) throw new Error('Ein Tabellenblatt überschreitet 20.000 Zeilen.');
    const result: WorkbookSheet = { name: sheet.getAttribute('name') || 'Tabelle', rows: [] };
    for (const row of rows) {
      const cells: Record<string, string> = Object.create(null);
      const number = Number(row.getAttribute('r'));
      if (!Number.isInteger(number) || number < 1 || number > 1000000)
        throw new Error('XLSX-Zeilennummer ungültig.');
      for (const cell of children(row, 'c')) {
        if (++cellCount > 250000) throw new Error('Die Arbeitsmappe enthält zu viele Zellen.');
        const address = cell.getAttribute('r') || '';
        if (!/^[A-Z]{1,3}[0-9]{1,7}$/.test(address)) throw new Error('XLSX-Zelladresse ungültig.');
        // Formulas remain inert and are not used as confirmed requirements, even with cached values.
        if (first(cell, 'f')) {
          cells[column(address)] = '[Formel nicht ausgewertet]';
          continue;
        }
        const kind = cell.getAttribute('t');
        let content = value(cell, 'v');
        if (kind === 's') {
          const index = Number(content);
          if (!/^\d+$/.test(content) || index >= strings.length)
            throw new Error('XLSX-Textverweis ungültig.');
          content = strings[index]!;
        } else if (kind === 'inlineStr') {
          content = Array.from(cell.getElementsByTagNameNS('*', 't'))
            .map((t) => t.textContent || '')
            .join('');
        }
        if (content.length > 16000)
          throw new Error('Eine Excel-Zelle überschreitet 16.000 Zeichen.');
        if (content.trim()) cells[column(address)] = content.trim();
      }
      if (Object.keys(cells).length) result.rows.push({ number, cells });
    }
    // Merged labels are presentation. Only explicit cells become field occurrences.
    return result;
  });
}

function headerRow(sheet: WorkbookSheet) {
  return [...sheet.rows.slice(0, 30)].sort((a, b) => {
    const score = (row: typeof a) =>
      Object.values(row.cells).filter((text) =>
        /feld.*id|^id\b|formularinhalt|bezeichnung|abbildung|zielpfad|target.*path/i.test(text),
      ).length;
    return score(b) - score(a) || a.number - b.number;
  })[0];
}
export function detectColumns(sheet: WorkbookSheet): RequirementColumns {
  const header = headerRow(sheet)?.cells || {};
  const entries = Object.entries(header);
  const find = (pattern: RegExp) =>
    entries.find(([, text]) => pattern.test(clean(text)))?.[0] || '';
  const path = entries
    .filter(([, text]) =>
      /abbildung|^nachricht$|^ebene\b|zielpfad|ziel.*ebene|target.*path|xpath/.test(clean(text)),
    )
    .map(([col]) => col);
  return {
    id: find(/^id\b|feld.*kennung|feld.*id|fim.*id/),
    label: find(/formularinhalt|bezeichnung|beschriftung|quell.*name|feldname/),
    path: path.filter((col) => !/\btyp\b|datentyp/.test(clean(header[col]!))),
    type: find(/\btyp\b|datentyp/),
    note: find(/anmerkung|bemerkung|hinweis|kommentar/),
    codelist: find(/codelisten.*zuordnung|codelist.*mapping/),
  };
}
export function selectRequirementSheet(sheets: WorkbookSheet[]) {
  return (
    [...sheets].sort((a, b) => {
      const score = (s: WorkbookSheet) => (detectColumns(s).id ? 100000 : 0) + s.rows.length;
      return score(b) - score(a);
    })[0]?.name || ''
  );
}
export function requirementsFromSheet(
  name: string,
  sheets: WorkbookSheet[],
  sheetName: string,
  columns?: RequirementColumns,
): Requirements {
  const sheet = sheets.find((s) => s.name === sheetName);
  if (!sheet) throw new Error('Tabellenblatt auswählen.');
  const cols = columns || detectColumns(sheet);
  if (!cols.id || !cols.label || !cols.path.length)
    throw new Error('Spalten für Feldkennung, Beschriftung und Zielpfad zuordnen.');
  const fields: Requirement[] = [],
    constants: Requirements['constants'] = [],
    warnings: string[] = [];
  let group = '',
    active: Requirement | undefined;
  for (const row of sheet.rows.filter((row) => row.number > (headerRow(sheet)?.number || 0))) {
    const id = row.cells[cols.id] || '',
      label = row.cells[cols.label] || '';
    if (/^G\d+/i.test(id)) {
      group = label;
      active = undefined;
      continue;
    }
    if (/^C\d+/i.test(id)) {
      if (active) active.codelist.push({ value: label, target: row.cells[cols.codelist] || '' });
      continue;
    }
    if (!id || !label) continue;
    const rawPath = cols.path.map((c) => row.cells[c] || '').filter(Boolean);
    const path =
      rawPath.length === 1 && rawPath[0]!.includes('/')
        ? rawPath[0]!.split('/').filter(Boolean)
        : rawPath;
    if (!path.length || rawPath.some((p) => p.includes('[Formel'))) continue;
    if (path.some((p) => p === '…' || p === '...'))
      warnings.push(`Unvollständiger Zielpfad in ${sheet.name}, Zeile ${row.number}.`);
    active = {
      id,
      label,
      path,
      group,
      type: row.cells[cols.type] || '',
      note: row.cells[cols.note] || '',
      sheet: sheet.name,
      row: row.number,
      cells: [cols.id, cols.label, ...cols.path].map((c) => c + row.number),
      codelist: [],
    };
    fields.push(active);
    if (fields.length > 2000)
      throw new Error('Höchstens 2.000 Feldanforderungen werden unterstützt.');
  }
  for (const constantsSheet of sheets.filter((s) => /konstant|constant/i.test(s.name))) {
    const header = constantsSheet.rows[0]?.cells || {};
    const pathCols = Object.entries(header)
      .filter(([, v]) => /abbildung|zielpfad|ebene/i.test(v))
      .map(([c]) => c);
    const content = Object.entries(header).find(([, v]) =>
      /übermittelter inhalt|konstante|wert/i.test(v),
    )?.[0];
    if (!content) continue;
    for (const row of constantsSheet.rows.slice(1)) {
      const path = pathCols.map((c) => row.cells[c] || '').filter(Boolean);
      if (row.cells[content] && path.length)
        constants.push({
          value: row.cells[content]!,
          path,
          sheet: constantsSheet.name,
          row: row.number,
        });
    }
  }
  if (!fields.length) throw new Error('Keine Feldanforderungen in den gewählten Spalten gefunden.');
  if (
    sheets.some((s) =>
      s.rows.some((r) => Object.values(r.cells).includes('[Formel nicht ausgewertet]')),
    )
  )
    warnings.push('Formelzellen wurden nicht ausgewertet und liefern keine bestätigte Zuordnung.');
  if (constants.length)
    warnings.push(
      constants.length === 1
        ? 'Eine dokumentierte Konstante ist separat zu prüfen.'
        : `${constants.length} dokumentierte Konstanten sind separat zu prüfen.`,
    );
  return {
    name,
    sheet: sheet.name,
    namespace:
      sheets
        .filter((s) => /header|kopf/i.test(s.name))
        .flatMap((s) => s.rows.flatMap((r) => Object.values(r.cells)))
        .map((v) => v.match(/^xmlns\s*=\s*["']([^"']+)/)?.[1] || '')
        .find(Boolean) || '',
    columns: cols,
    fields,
    constants,
    warnings: [...new Set(warnings)],
  };
}
