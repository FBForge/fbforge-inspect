import { readFile } from 'node:fs/promises';
import { test, expect, type Page } from './fixtures';
import { selectAppearance } from './theme';

const root = 'tests/fixtures/mapping/';
async function start(page: Page) {
  await page.goto('/');
  await page.getByRole('button', { name: 'Automatisch zuordnen', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Strukturen automatisch zuordnen' }),
  ).toBeVisible();
}
async function schemas(page: Page) {
  await page.getByLabel('Quelle öffnen', { exact: true }).setInputFiles(root + 'source.xsd');
  await page.getByLabel('Ziel öffnen', { exact: true }).setInputFiles(root + 'target.xsd');
  await expect(
    page.getByRole('button', { name: 'Automatische Zuordnung starten', exact: true }),
  ).toBeEnabled();
}
async function run(page: Page) {
  await page.getByRole('button', { name: 'Automatische Zuordnung starten', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Zuordnungen prüfen', exact: true }),
  ).toBeVisible();
  await expect(page.locator('.mapping-row h3').first()).toBeInViewport();
}
async function all(page: Page) {
  await page
    .getByRole('group', { name: 'Zuordnungen filtern' })
    .getByRole('button', { name: /^Alle Zuordnungen/ })
    .click();
}
test('optional requirements disambiguate a field and preserve many-to-one conflicts', async ({
  page,
}) => {
  await start(page);
  await schemas(page);
  await run(page);
  await all(page);
  let family = page
    .locator('.mapping-row')
    .filter({ has: page.getByRole('heading', { name: 'Familienname', exact: true }) });
  await expect(family).toContainText('Bitte entscheiden');
  await page
    .getByLabel('Excel-Anforderungen öffnen', { exact: true })
    .setInputFiles(root + 'requirements.xlsx');
  await expect(page.getByRole('heading', { name: 'Zuordnungen prüfen', exact: true })).toHaveCount(
    0,
  );
  await expect(
    page.getByText('5 Feldanforderungen · 1 dokumentierte Konstante', { exact: true }),
  ).toBeVisible();
  await run(page);
  await all(page);
  family = page
    .locator('.mapping-row')
    .filter({ has: page.getByRole('heading', { name: 'Familienname', exact: true }) });
  await expect(family).toContainText('Automatisch zugeordnet');
  await expect(family).toContainText('Ziel / Person / Nachname');
  await family
    .getByRole('button', { name: 'Begründung und Excel-Fundstelle', exact: true })
    .click();
  await expect(family).toContainText('Fiktive Anforderungen, Zeile 3');
  const street = page
    .locator('.mapping-row')
    .filter({ has: page.getByRole('heading', { name: 'Straße', exact: true }) });
  await expect(street).toContainText('Mehrere Quellfelder zeigen auf dasselbe Ziel');
  const house = page
    .locator('.mapping-row')
    .filter({ has: page.getByRole('heading', { name: 'Hausnummer', exact: true }) });
  await house.getByRole('button', { name: 'Feld nicht zuordnen', exact: true }).click();
  await expect(house).toContainText('Nicht zugeordnet');
  await street.getByRole('button', { name: 'Zuordnung bestätigen', exact: true }).click();
  await expect(street).toContainText('Zuordnung bestätigt');
  await house.getByRole('button', { name: 'Erneut zuordnen', exact: true }).click();
  await expect(house).toContainText('Bitte entscheiden');
  await page.getByLabel('Zuordnungen durchsuchen').fill('Geburtsdatum');
  await expect(page.locator('.mapping-row')).toHaveCount(1);
  await page.getByLabel('Zuordnungen durchsuchen').fill('');
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Start', exact: true })
    .click();
  await page.getByRole('button', { name: 'Automatisch zuordnen', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Zuordnungen prüfen', exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Excel entfernen', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Zuordnungen prüfen', exact: true })).toHaveCount(
    0,
  );
  await run(page);
  await all(page);
  await expect(
    page
      .locator('.mapping-row')
      .filter({ has: page.getByRole('heading', { name: 'Familienname', exact: true }) }),
  ).toContainText('Bitte entscheiden');
});

test('mapping export produces a binary ZIP with a self-contained MapForce 2025 draft', async ({
  page,
}) => {
  await page.addInitScript(() =>
    Object.assign(window, {
      isTauri: true,
      __saved: [],
      __TAURI_INTERNALS__: {
        invoke: async (command: string, args: any) => {
          if (command === 'save_report') {
            (window as any).__saved.push(args);
            return true;
          }
          return false;
        },
      },
    }),
  );
  await start(page);
  await schemas(page);
  await page
    .getByLabel('Excel-Anforderungen öffnen', { exact: true })
    .setInputFiles(root + 'requirements-stored.xlsx');
  await run(page);
  await page.getByRole('button', { name: 'Zuordnungsbericht speichern', exact: true }).click();
  await expect(page.getByText('Zuordnungsbericht gespeichert.', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'MapForce-Entwurf exportieren', exact: true }).click();
  await expect(
    page.getByText('MapForce-Entwurf als ZIP gespeichert.', { exact: true }),
  ).toBeVisible();
  const result = await page.evaluate(async () => {
    const args = (window as any).__saved.at(-1);
    const zip = Uint8Array.from(atob(args.content), (c) => c.charCodeAt(0));
    const { readZip } = await import('/src/mapping/zip.ts');
    // Native payload is binary ZIP. Inspect its stored entries independently of MIME labels.
    const view = new DataView(zip.buffer);
    let at = 0;
    const entries: Record<string, string> = {};
    while (view.getUint32(at, true) === 0x04034b50) {
      const size = view.getUint32(at + 18, true),
        nameLength = view.getUint16(at + 26, true),
        extra = view.getUint16(at + 28, true),
        name = new TextDecoder().decode(zip.slice(at + 30, at + 30 + nameLength)),
        start = at + 30 + nameLength + extra;
      entries[name] = new TextDecoder().decode(zip.slice(start, start + size));
      at = start + size;
    }
    const mfd = new DOMParser().parseFromString(entries['zuordnung.mfd']!, 'application/xml');
    const declared = new Set(
      [...mfd.querySelectorAll('[inpkey],[outkey]')].flatMap((e) =>
        [e.getAttribute('inpkey'), e.getAttribute('outkey')].filter(Boolean),
      ),
    );
    const connected = [...mfd.querySelectorAll('vertex,edge')].every((e) =>
      declared.has(e.getAttribute('vertexkey')),
    );
    void readZip;
    return {
      format: args.format,
      names: Object.keys(entries),
      version: mfd.documentElement.getAttribute('version'),
      connected,
      report: JSON.parse(entries['zuordnung.json']!),
      readme: entries['README.txt'],
    };
  });
  expect(result.format).toBe('zip');
  expect(result.names).toContain('zuordnung.mfd');
  expect(result.names).toContain('quelle-01.xsd');
  expect(result.names).toContain('ziel-01.xsd');
  expect(result.version).toBe('22');
  expect(result.connected).toBeTruthy();
  expect(result.report.mapforce.targetVersion).toBe('2025');
  expect(result.report.mapforce.exportedDirectMappings).toBe(3);
  expect(result.report.mapforce.notExported).toHaveLength(2);
  expect(result.readme).toContain('müssen in Ihrer MapForce-2025-Installation geprüft werden');
});

test('XDF can use extra requirements while its original context is retained', async ({ page }) => {
  await start(page);
  await page.getByLabel('Quelle öffnen', { exact: true }).setInputFiles(root + 'source.xdf');
  await page.getByLabel('Ziel öffnen', { exact: true }).setInputFiles(root + 'target.xsd');
  await page
    .getByLabel('Excel-Anforderungen öffnen', { exact: true })
    .setInputFiles(root + 'requirements.xlsx');
  await run(page);
  await all(page);
  await expect(page.locator('.mapping-row')).toHaveCount(1);
  await expect(page.locator('.mapping-row')).toContainText(
    'S90000001V1.0 / G90000001V1.0 / F90000001V1.0',
  );
  await expect(page.locator('.mapping-row')).toContainText('Automatisch zugeordnet');
  await expect(
    page.getByRole('button', { name: 'MapForce-Entwurf exportieren', exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByText(/Für den MapForce-Entwurf werden Quell- und Ziel-XSD benötigt/),
  ).toBeVisible();
});

test('malformed inputs never retain exportable results and columns remain adjustable', async ({
  page,
}) => {
  await start(page);
  await schemas(page);
  await run(page);
  await page.getByLabel('Excel-Anforderungen öffnen', { exact: true }).setInputFiles({
    name: 'broken.xlsx',
    mimeType: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
    buffer: Buffer.from('not a workbook'),
  });
  await expect(page.getByRole('alert')).toContainText('XLSX');
  await expect(page.getByRole('heading', { name: 'Zuordnungen prüfen', exact: true })).toHaveCount(
    0,
  );
  await page
    .getByLabel('Excel-Anforderungen öffnen', { exact: true })
    .setInputFiles(root + 'requirements.xlsx');
  await page.getByRole('button', { name: 'Erkannte Spalten und Vorschau', exact: true }).click();
  await page.getByLabel('Spalte Feldkennung', { exact: true }).fill('');
  await page.getByRole('button', { name: 'Spaltenzuordnung übernehmen', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Spalten');
  await expect(
    page.getByRole('button', { name: 'Automatische Zuordnung starten', exact: true }),
  ).toBeDisabled();
  await page.getByLabel('Spalte Feldkennung', { exact: true }).fill('b');
  await page.getByRole('button', { name: 'Spaltenzuordnung übernehmen', exact: true }).click();
  await run(page);
  await page.getByLabel('Ziel öffnen', { exact: true }).setInputFiles({
    name: 'bad.xsd',
    mimeType: 'application/xml',
    buffer: Buffer.from('<!DOCTYPE schema [<!ENTITY x "fixture">]><schema/>'),
  });
  await expect(page.getByRole('alert')).toContainText('DTD');
  await expect(page.getByRole('heading', { name: 'Zuordnungen prüfen', exact: true })).toHaveCount(
    0,
  );
  await page.getByRole('button', { name: 'Quelle entfernen', exact: true }).click();
  await expect(
    page.getByRole('button', { name: 'Automatische Zuordnung starten', exact: true }),
  ).toBeDisabled();
});

test('mapping reflows in both appearances and screenshots show synthetic requirements', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1360, height: 1000 });
  await start(page);
  await selectAppearance(page, 'light');
  await schemas(page);
  await page
    .getByLabel('Excel-Anforderungen öffnen', { exact: true })
    .setInputFiles(root + 'requirements.xlsx');
  if (process.env.INSPECT_UPDATE_DOC_IMAGES === '1') {
    await expect(
      page.getByText('5 Feldanforderungen · 1 dokumentierte Konstante', { exact: true }),
    ).toBeVisible();
    await page.setViewportSize({ width: 1360, height: 1100 });
    await page.screenshot({ path: 'docs/images/mapping-inputs.png' });
  }
  await page.setViewportSize({ width: 1360, height: 1000 });
  await run(page);
  await page.screenshot({
    path:
      process.env.INSPECT_UPDATE_DOC_IMAGES === '1'
        ? 'docs/images/mapping.png'
        : '.local/quality/screenshots/mapping.png',
  });
  await selectAppearance(page, 'dark');
  await page.screenshot({ path: '.local/quality/screenshots/mapping-dark.png' });
  await page.setViewportSize({ width: 700, height: 650 });
  await page.evaluate(() => {
    const sizes = [...document.querySelectorAll<HTMLElement>('body *')].map((e) => ({
      e,
      size: getComputedStyle(e).fontSize,
    }));
    for (const { e, size } of sizes) e.style.fontSize = `${parseFloat(size) * 2}px`;
  });
  expect(
    await page.locator('main').evaluate((e) => e.scrollWidth <= e.clientWidth + 1),
  ).toBeTruthy();
  // Ubuntu runners commonly select DejaVu Sans. Its wider glyphs must also fit.
  await page.evaluate(() =>
    document.documentElement.style.setProperty('--font-ui', '"DejaVu Sans", sans-serif'),
  );
  expect(
    await page.locator('main').evaluate((e) => e.scrollWidth <= e.clientWidth + 1),
  ).toBeTruthy();
  expect(
    await page
      .locator('.mapping-export .button')
      .evaluateAll((buttons) => buttons.every((e) => e.scrollWidth <= e.clientWidth + 1)),
  ).toBeTruthy();
  await page.setViewportSize({ width: 350, height: 540 });
  expect(
    await page.locator('main').evaluate((e) => e.scrollWidth <= e.clientWidth + 1),
  ).toBeTruthy();
});

test('schema structures resolve local types and retain cardinality and namespace boundaries', async ({
  page,
}) => {
  await page.goto('/');
  const result = await page.evaluate(async () => {
    const { readSchema } = await import('/src/mapping/schema.ts');
    const main = `<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:t="urn:target" targetNamespace="urn:target" elementFormDefault="qualified"><xs:include schemaLocation="types.xsd"/><xs:element name="Root" type="t:Person"/><xs:element name="Other" type="xs:string"/></xs:schema>`;
    const types = `<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:t="urn:target" targetNamespace="urn:target" elementFormDefault="qualified"><xs:complexType name="Base"><xs:sequence><xs:element name="Name" type="xs:string"/></xs:sequence></xs:complexType><xs:complexType name="Person"><xs:complexContent><xs:extension base="t:Base"><xs:sequence><xs:element name="Alias" type="xs:string" minOccurs="0" maxOccurs="unbounded"/></xs:sequence><xs:attribute name="code" type="xs:string" use="required"/></xs:extension></xs:complexContent></xs:complexType></xs:schema>`;
    const docs = [
      { name: 'main.xsd', text: main },
      { name: 'types.xsd', text: types },
    ];
    const full = readSchema(docs),
      missing = readSchema([docs[0]!]),
      other = readSchema(docs, 'Other');
    const json = readSchema([
      {
        name: 'schema.json',
        text: JSON.stringify({
          type: 'object',
          required: ['person'],
          properties: {
            person: { $ref: '#/$defs/person' },
            list: { type: 'array', items: { type: 'string' } },
          },
          $defs: {
            person: {
              type: 'object',
              required: ['name'],
              properties: { name: { type: 'string', title: 'Name' } },
            },
          },
        }),
      },
    ]);
    return {
      fields: full.fields.map((f) => ({
        path: f.path,
        type: f.type,
        attribute: f.attribute,
        required: f.required,
        repeated: f.repeated,
        namespace: f.namespace,
      })),
      warnings: full.warnings,
      missing: missing.warnings,
      other: other.fields.length,
      json: json.fields.map((f) => ({ path: f.path, required: f.required, repeated: f.repeated })),
    };
  });
  expect(result.fields).toHaveLength(3);
  expect(result.warnings).toEqual([]);
  expect(result.fields.find((f) => f.path.at(-1) === 'Alias')).toMatchObject({
    repeated: true,
    required: false,
  });
  expect(result.fields.find((f) => f.attribute)).toMatchObject({ namespace: '', required: true });
  expect(result.missing.join(' ')).toContain('Begleitschema fehlt');
  expect(result.other).toBe(1);
  expect(result.json).toHaveLength(2);
});

test('OOXML import retains actual field rows, groups and constants', async ({ page }) => {
  const fixture = await readFile(root + 'requirements.xlsx');
  await page.goto('/');
  const result = await page.evaluate(
    async (bytes) => {
      const { readWorkbook, requirementsFromSheet } = await import('/src/mapping/requirements.ts');
      const sheets = await readWorkbook(new Uint8Array(bytes));
      const parsed = requirementsFromSheet('synthetic.xlsx', sheets, 'Fiktive Anforderungen');
      return { count: parsed.fields.length, first: parsed.fields[0], constants: parsed.constants };
    },
    [...fixture],
  );
  expect(result.count).toBe(5);
  expect(result.first).toMatchObject({
    id: 'F90000001V1.0 (1:1)',
    group: 'Person',
    row: 3,
    path: ['Ziel', 'Person', 'Nachname'],
  });
  expect(result.constants).toHaveLength(1);
});

test('manual target search and pending column edits cannot silently change saved decisions', async ({
  page,
}) => {
  await start(page);
  await schemas(page);
  await page
    .getByLabel('Excel-Anforderungen öffnen', { exact: true })
    .setInputFiles(root + 'requirements.xlsx');
  await run(page);
  await all(page);
  const family = page
    .locator('.mapping-row')
    .filter({ has: page.getByRole('heading', { name: 'Familienname', exact: true }) });
  await family.getByRole('button', { name: 'Anderes Zielfeld suchen', exact: true }).click();
  await family.getByLabel('Zielfeld suchen', { exact: true }).fill('Kontakt / Email');
  await family.getByRole('button', { name: 'Ziel / Kontakt / Email', exact: true }).click();
  await expect(family.getByLabel('Zielfeld auswählen', { exact: true })).toHaveValue(/Email$/);
  await expect(family.getByText('Ziel: Ziel / Person / Nachname', { exact: true })).toBeVisible();
  await family.getByRole('button', { name: 'Zuordnung bestätigen', exact: true }).click();
  await expect(family).toContainText('Mehrere Quellfelder zeigen auf dasselbe Ziel');
  await page.getByRole('button', { name: /^Zielfelder ohne Vorschlag/ }).click();
  await expect(
    page.getByText('Ziel / Person / Nachname · erforderlich', { exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Erkannte Spalten und Vorschau', exact: true }).click();
  await page.getByLabel('Spalte Feldkennung', { exact: true }).fill('A');
  await expect(
    page.getByRole('button', { name: 'Zuordnungsbericht speichern', exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole('button', { name: 'Automatische Zuordnung starten', exact: true }),
  ).toBeDisabled();
  await page.getByLabel('Spalte Feldkennung', { exact: true }).fill('b');
  await page.getByRole('button', { name: 'Spaltenzuordnung übernehmen', exact: true }).click();
  await run(page);
  await expect(
    page.getByRole('button', { name: 'Zuordnungsbericht speichern', exact: true }),
  ).toBeVisible();
});

test('browser ZIP download contains binary archive bytes and native save failures stay actionable', async ({
  page,
}) => {
  await start(page);
  await schemas(page);
  await run(page);
  const downloadPromise = page.waitForEvent('download');
  await page.getByRole('button', { name: 'MapForce-Entwurf exportieren', exact: true }).click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toBe('fbforge-inspect.zip');
  const bytes = await readFile((await download.path())!);
  expect(bytes.readUInt32LE(0)).toBe(0x04034b50);
  await page.evaluate(() =>
    Object.assign(window, {
      isTauri: true,
      __TAURI_INTERNALS__: {
        invoke: async () => {
          throw new Error('Speicherziel nicht verfügbar');
        },
      },
    }),
  );
  await page.getByRole('button', { name: 'Zuordnungsbericht speichern', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Speicherziel nicht verfügbar');
  await expect(
    page.getByRole('button', { name: 'Zuordnungsbericht speichern', exact: true }),
  ).toBeEnabled();
});

test('review actions and file removal preserve keyboard focus when the current control disappears', async ({
  page,
}) => {
  await start(page);
  await schemas(page);
  await page
    .getByLabel('Excel-Anforderungen öffnen', { exact: true })
    .setInputFiles(root + 'requirements.xlsx');
  await run(page);
  const street = page
    .locator('.mapping-row')
    .filter({ has: page.getByRole('heading', { name: 'Straße', exact: true }) });
  await street.getByRole('button', { name: 'Feld nicht zuordnen', exact: true }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Hausnummer', exact: true })).toBeFocused();
  await page.getByRole('button', { name: 'Zuordnung bestätigen', exact: true }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('button', { name: 'Offene Fälle · 0', exact: true })).toBeFocused();
  await page.getByRole('button', { name: 'Ausgelassen · 1', exact: true }).click();
  await page.getByRole('button', { name: 'Erneut zuordnen', exact: true }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('button', { name: 'Ausgelassen · 0', exact: true })).toBeFocused();
  await page.getByRole('button', { name: 'Excel entfernen', exact: true }).focus();
  await page.keyboard.press('Enter');
  await expect(
    page.getByRole('button', { name: 'Excel-Anforderungen öffnen', exact: true }),
  ).toBeFocused();
  await page.getByRole('button', { name: 'Quelle entfernen', exact: true }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('button', { name: 'Quelle öffnen', exact: true })).toBeFocused();
});
