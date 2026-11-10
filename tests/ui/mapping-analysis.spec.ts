import { test, expect } from './fixtures';

test('requirements preserve reused field contexts, code lists, formulas and explicit namespace', async ({
  page,
}) => {
  await page.goto('/');
  const result = await page.evaluate(async () => {
    const { zipFiles } = await import('/src/mapping/zip.ts');
    const { readWorkbook, requirementsFromSheet, detectColumns, selectRequirementSheet } =
      await import('/src/mapping/requirements.ts');
    const sheet = `<worksheet><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Hinweise</t></is></c></row><row r="3"><c r="B3" t="s"><v>0</v></c><c r="C3" t="s"><v>1</v></c><c r="D3" t="s"><v>2</v></c><c r="E3" t="inlineStr"><is><t>Codelisten-Zuordnung</t></is></c></row><row r="4"><c r="B4" t="inlineStr"><is><t>G90000001</t></is></c><c r="C4" t="inlineStr"><is><t>Mutter</t></is></c></row><row r="5"><c r="B5" t="inlineStr"><is><t>F90000001</t></is></c><c r="C5" t="inlineStr"><is><t>Name</t></is></c><c r="D5" t="inlineStr"><is><t>Root/Mutter/Name</t></is></c></row><row r="6"><c r="B6" t="inlineStr"><is><t>C90000001</t></is></c><c r="C6" t="inlineStr"><is><t>Fiktiv</t></is></c><c r="E6"><v>1</v></c></row><row r="7"><c r="B7" t="inlineStr"><is><t>G90000002</t></is></c><c r="C7" t="inlineStr"><is><t>Vater</t></is></c></row><row r="8"><c r="B8" t="inlineStr"><is><t>F90000001</t></is></c><c r="C8" t="inlineStr"><is><t>Name</t></is></c><c r="D8" t="inlineStr"><is><t>Root/Vater/Name</t></is></c></row><row r="9"><c r="B9" t="inlineStr"><is><t>F90000002</t></is></c><c r="C9" t="inlineStr"><is><t>Formel</t></is></c><c r="D9"><f>HYPERLINK("https://invalid.example")</f><v>Root/Name</v></c></row></sheetData></worksheet>`;
    const packageFiles = [
      { name: '[Content_Types].xml', content: '<Types/>' },
      {
        name: 'xl/workbook.xml',
        content:
          '<workbook xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Anforderungen" r:id="r1"/></sheets></workbook>',
      },
      {
        name: 'xl/_rels/workbook.xml.rels',
        content:
          '<Relationships><Relationship Id="r1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="custom" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml" Target="../customXml/item1.xml"/></Relationships>',
      },
      {
        name: 'xl/sharedStrings.xml',
        content:
          '<sst><si><t>ID</t></si><si><r><t>Formular</t></r><r><t>inhalt</t></r></si><si><t>Zielpfad</t></si></sst>',
      },
      { name: 'xl/worksheets/sheet1.xml', content: sheet },
    ];
    // ZIP writer intentionally only emits flat generated export names. For this
    // parser scenario use its valid ZIP records, then patch equal-length names.
    const make = (files: typeof packageFiles) => {
      const names = files.map((f, i) => ({
        original: f.name,
        safe: 'x'.repeat(f.name.length - String(i).length) + i,
      }));
      const bytes = zipFiles(files.map((f, i) => ({ name: names[i]!.safe, content: f.content })));
      for (const name of names) {
        const needle = new TextEncoder().encode(name.safe),
          replacement = new TextEncoder().encode(name.original);
        for (let i = 0; i <= bytes.length - needle.length; i++)
          if (needle.every((b, j) => bytes[i + j] === b)) bytes.set(replacement, i);
      }
      return bytes;
    };
    const sheets = await readWorkbook(make(packageFiles));
    sheets.push({
      name: 'Header',
      rows: [{ number: 1, cells: { B: 'xmlns="urn:example:2025"' } }],
    });
    const parsed = requirementsFromSheet('synthetic.xlsx', sheets, selectRequirementSheet(sheets));
    const failures: string[] = [];
    for (const replacement of [
      { index: 0, text: '<Types><Override ContentType="vbaProject"/></Types>' },
      {
        index: 2,
        text: packageFiles[2]!.content.replace(
          'Target="worksheets',
          'TargetMode="External" Target="worksheets',
        ),
      },
      { index: 4, text: sheet.replace('r="B5"', 'r="invalid"') },
      { index: 4, text: sheet.replace('<v>0</v>', '<v>999</v>') },
      { index: 4, text: sheet.replace('r="1"', 'r="0"') },
    ]) {
      const files = packageFiles.map((f, i) => ({
        ...f,
        content: i === replacement.index ? replacement.text : f.content,
      }));
      try {
        await readWorkbook(make(files));
        failures.push('accepted');
      } catch (e) {
        failures.push(String(e));
      }
    }
    return {
      parsed,
      columns: detectColumns(sheets[0]!),
      formula: sheets[0]!.rows.at(-1),
      failures,
    };
  });
  expect(result.parsed.fields).toHaveLength(2);
  expect(result.parsed.fields.map((f) => f.group)).toEqual(['Mutter', 'Vater']);
  expect(result.parsed.fields[0]!.codelist).toEqual([{ value: 'Fiktiv', target: '1' }]);
  expect(result.parsed.namespace).toBe('urn:example:2025');
  expect(result.parsed.warnings.join(' ')).toContain('Formelzellen');
  expect(result.formula!.cells.D).toBe('[Formel nicht ausgewertet]');
  expect(result.columns.path).toEqual(['D']);
  expect(result.failures.every((f) => f !== 'accepted')).toBe(true);
});

test('matching keeps incompatible types, roles, namespaces and uncertain evidence open', async ({
  page,
}) => {
  await page.goto('/');
  const result = await page.evaluate(async () => {
    const { readSchema } = await import('/src/mapping/schema.ts');
    const { suggestMappings, chooseMapping, markConflicts } =
      await import('/src/mapping/matcher.ts');
    const schema = (properties: any) =>
      readSchema([{ name: 'sample.json', text: JSON.stringify({ type: 'object', properties }) }]);
    const source = schema({
      Mutter: { type: 'object', required: ['Name'], properties: { Name: { type: 'string' } } },
    });
    const target = schema({
      Vater: { type: 'object', required: ['Name'], properties: { Name: { type: 'integer' } } },
    });
    const mismatch = await suggestMappings(source, target, null);
    chooseMapping(mismatch[0]!, target.fields[0]!, source.fields[0]!);
    const grouped = schema({
      Mutter: { type: 'object', properties: { F90000001: { type: 'string', title: 'Name' } } },
      Vater: { type: 'object', properties: { F90000001: { type: 'string', title: 'Name' } } },
    });
    const dest = schema({
      Root: {
        type: 'object',
        properties: {
          Mutter: { type: 'object', properties: { Name: { type: 'string' } } },
          Vater: { type: 'object', properties: { Name: { type: 'string' } } },
        },
      },
    });
    const requirements: any = {
      name: 'sample.xlsx',
      sheet: 'Fields',
      namespace: '',
      columns: {},
      constants: [],
      warnings: [],
      fields: ['Mutter', 'Vater'].map((group, i) => ({
        id: 'F90000001',
        label: 'Name',
        path: ['Root', group, 'Name'],
        group,
        type: 'string',
        note: '',
        sheet: 'Fields',
        row: i + 1,
        cells: ['A' + (i + 1)],
        codelist: [],
      })),
    };
    const contexts = await suggestMappings(grouped, dest, requirements);
    requirements.namespace = 'urn:wrong';
    const namespace = await suggestMappings(grouped, dest, requirements);
    const typed = await suggestMappings(grouped, dest, {
      ...requirements,
      namespace: '',
      fields: requirements.fields.map((r: any) => ({ ...r, type: 'integer' })),
    });
    const unresolved = await suggestMappings(grouped, dest, {
      ...requirements,
      namespace: '',
      fields: requirements.fields.map((r: any) => ({ ...r, path: ['Unknown', 'Name'] })),
    });
    const cancelled = new AbortController();
    cancelled.abort();
    let cancel = '';
    try {
      await suggestMappings(source, target, null, cancelled.signal);
    } catch (e) {
      cancel = String(e);
    }
    const ids = schema({ G90000001: { type: 'string' }, G90000002: { type: 'string' } });
    const ids2 = schema({ G90000003: { type: 'string' } });
    const groupIds = await suggestMappings(ids, ids2, null);
    const constraints = await suggestMappings(
      schema({ Name: { type: 'string' } }),
      schema({ Name: { type: 'string', maxLength: 4 } }),
      null,
    );
    const optional = await suggestMappings(
      schema({ Name: { type: 'string' } }),
      readSchema([
        {
          name: 'required.json',
          text: JSON.stringify({
            type: 'object',
            required: ['Name'],
            properties: { Name: { type: 'string' } },
          }),
        },
      ]),
      null,
    );
    return {
      mismatch,
      contexts,
      namespace,
      typed,
      unresolved,
      cancel,
      groupIds,
      constraints,
      optional,
      stable: markConflicts(mismatch),
    };
  });
  expect(result.mismatch[0]!.warnings.join(' ')).toContain('Typumwandlung');
  expect(result.mismatch[0]!.warnings.join(' ')).toContain('Personenrollen');
  expect(result.contexts.every((d) => d.state === 'automatic')).toBe(true);
  expect(new Set(result.contexts.map((d) => d.target)).size).toBe(2);
  expect(result.namespace.every((d) => d.state === 'review')).toBe(true);
  expect(result.namespace[0]!.warnings.join(' ')).toContain('Excel-Namespace');
  expect(result.unresolved.every((d) => d.state === 'review')).toBe(true);
  expect(result.typed.every((d) => d.state === 'review')).toBe(true);
  expect(result.typed[0]!.warnings.join(' ')).toContain('Excel-Datentyp');
  expect(result.cancel).toContain('abgebrochen');
  expect(result.groupIds.every((d) => d.reasons.every((r) => !r.includes('Feldkennung')))).toBe(
    true,
  );
  expect(result.constraints[0]!.state).toBe('review');
  expect(result.optional[0]!.warnings.join(' ')).toContain('Optionales');
});

test('local schema analysis handles groups, choices, simple content and unresolved references', async ({
  page,
}) => {
  await page.goto('/');
  const result = await page.evaluate(async () => {
    const { readSchema } = await import('/src/mapping/schema.ts');
    const wrap = (body: string) =>
      `<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">${body}</xs:schema>`;
    const parsed = readSchema([
      {
        name: 'groups.xsd',
        text: wrap(
          '<xs:group name="Group"><xs:choice><xs:element name="Name" type="xs:string"/><xs:element name="Code"><xs:simpleType><xs:restriction base="xs:string"><xs:enumeration value="X"/></xs:restriction></xs:simpleType></xs:element></xs:choice></xs:group><xs:element name="Root"><xs:complexType><xs:sequence><xs:group ref="Group"/></xs:sequence><xs:attribute name="hidden" use="prohibited"/></xs:complexType></xs:element>',
        ),
      },
    ]);
    const simple = readSchema([
      {
        name: 'simple.xsd',
        text: wrap(
          '<xs:element name="Value"><xs:complexType><xs:simpleContent><xs:extension base="xs:date"><xs:attribute name="unit" type="xs:string"/></xs:extension></xs:simpleContent></xs:complexType></xs:element>',
        ),
      },
    ]);
    const recursive = readSchema([
      {
        name: 'recursive.xsd',
        text: wrap(
          '<xs:complexType name="Node"><xs:sequence><xs:element name="Label" type="xs:string"/><xs:element name="Child" type="Node" minOccurs="0"/></xs:sequence></xs:complexType><xs:element name="Root" type="Node"/>',
        ),
      },
    ]);
    const json = readSchema([
      {
        name: 'composed.json',
        text: JSON.stringify({
          properties: {
            missing: { $ref: '#/$defs/absent' },
            external: { $ref: 'https://invalid.example/schema' },
            loop: { $ref: '#/$defs/loop' },
            date: { type: 'string', format: 'date' },
            composed: { oneOf: [{ type: 'string' }, { type: 'integer' }] },
            list: { type: 'array', items: { type: 'integer' } },
          },
          $defs: { loop: { $ref: '#/$defs/loop' } },
        }),
      },
    ]);
    const errors: string[] = [];
    for (const docs of [
      [],
      [{ name: 'a', text: 'false' }],
      [{ name: 'a', text: '{' }],
      [{ name: 'a', text: '<other/>' }],
      [{ name: 'a', text: wrap('') }],
      [
        { name: 'a', text: wrap('<xs:element name="A" type="xs:string"/>') },
        { name: 'A', text: '{}' },
      ],
    ]) {
      try {
        readSchema(docs);
        errors.push('accepted');
      } catch (e) {
        errors.push(String(e));
      }
    }
    return { parsed, simple, recursive, json, errors };
  });
  expect(result.parsed.fields).toHaveLength(2);
  expect(result.parsed.fields.every((f) => f.warnings.join(' ').includes('Auswahl'))).toBe(true);
  expect(result.parsed.fields[1]!.warnings.join(' ')).toContain('Wertebeschränkungen');
  expect(result.simple.fields.find((f) => !f.attribute)!.type).toBe('date');
  expect(result.simple.fields.find((f) => f.attribute)!.names).toEqual(['Value', 'unit']);
  expect(result.recursive.warnings.join(' ')).toContain('Rekursiver');
  expect(result.json.warnings.join(' ')).toContain('Externe JSON-Referenz');
  expect(result.json.fields.find((f) => f.label === 'date')!.type).toBe('date');
  expect(result.json.fields.find((f) => f.label === 'list')!.repeated).toBe(true);
  expect(result.errors.every((e) => e !== 'accepted')).toBe(true);
});

test('archive import rejects damaged, encrypted, duplicate and oversized entries', async ({
  page,
}) => {
  await page.goto('/');
  const failures = await page.evaluate(async () => {
    const { readZip, zipFiles } = await import('/src/mapping/zip.ts');
    const file = zipFiles([{ name: 'valid.xml', content: '<valid/>' }]);
    const central = new DataView(file.buffer).getUint32(file.length - 6, true);
    const cases = [new Uint8Array(0), file.subarray(0, file.length - 1)];
    for (const [offset, value, width] of [
      [0, 0, 4],
      [central + 8, 1, 2],
      [central + 10, 99, 2],
      [central + 24, 13 * 1024 * 1024, 4],
      [central + 16, 0, 4],
      [central + 42, file.length, 4],
      [file.length - 12, 600, 2],
    ]) {
      const bad = file.slice(),
        view = new DataView(bad.buffer);
      if (width === 2) view.setUint16(offset!, value!, true);
      else view.setUint32(offset!, value!, true);
      cases.push(bad);
    }
    cases.push(
      zipFiles([
        { name: 'same.xml', content: '<a/>' },
        { name: 'same.xml', content: '<b/>' },
      ]),
    );
    const failures: string[] = [];
    for (const fixture of cases) {
      try {
        await readZip(fixture);
        failures.push('accepted');
      } catch (e) {
        failures.push(String(e));
      }
    }
    for (const files of [
      [{ name: '../file.xml', content: 'x' }],
      Array.from({ length: 129 }, (_, i) => ({ name: `file-${i}.xml`, content: '' })),
    ]) {
      try {
        zipFiles(files);
        failures.push('accepted');
      } catch (e) {
        failures.push(String(e));
      }
    }
    return failures;
  });
  expect(failures).toHaveLength(12);
  expect(failures.every((f) => f !== 'accepted')).toBe(true);
});

test('selected XDF main file uses explicit companion definitions and retains repeated occurrences', async ({
  page,
}) => {
  await page.goto('/');
  const result = await page.evaluate(async () => {
    const { readSchema } = await import('/src/mapping/schema.ts');
    const identify = (id: string) =>
      `<identifikation><id>${id}</id><version>1.0</version></identifikation>`;
    const documents = [
      {
        name: 'fields.xdf',
        text: `<xdf:datenschema xmlns:xdf="urn:xdatenfelder:3">${identify('S90000009')}<datenfeld>${identify('F90000001')}<name>Name</name><datentyp>text</datentyp></datenfeld></xdf:datenschema>`,
      },
      {
        name: 'main.xdf',
        text: `<xdf:stammdatenschema xmlns:xdf="urn:xdatenfelder:3">${identify('S90000001')}<name>Main</name><struktur><anzahl>1:10</anzahl><enthaelt><datenfeld>${identify('F90000001')}</datenfeld></enthaelt></struktur></xdf:stammdatenschema>`,
      },
    ];
    return readSchema(documents, '', 'main.xdf');
  });
  expect(result.root).toBe('S90000001V1.0');
  expect(result.fields).toHaveLength(1);
  expect(result.fields[0]!.names).toEqual(['S90000001V1.0', 'F90000001V1.0']);
  expect(result.fields[0]!.repeated).toBe(true);
});
