import { test, expect, type Page } from './fixtures';
const from = '00000000-0000-4000-8000-000000000001';
const to = '00000000-0000-4000-8000-000000000002';
async function go(page: Page, name: string) {
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name, exact: true })
    .click();
}
async function run(page: Page) {
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toBeVisible();
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const state = { calls: [] as any[] };
    Object.assign(window, {
      inspectUx: state,
      isTauri: true,
      __TAURI_INTERNALS__: {
        invoke: async (command: string, args: any) => {
          state.calls.push({ command, ...args });
          if (command === 'open_reference' || command === 'cancel') return;
          if (command === 'save_report') return false;
          const req = args.request;
          const f = (status: string, title: string, detail: string) => ({
            status,
            title,
            detail,
            path: '$',
            source: '',
            nextStep: '',
            limitation: '',
          });
          const data: any =
            req.operation === 'schema'
              ? {
                  dialect: 'JSON Schema',
                  structure: Array.from({ length: 250 }, (_, i) => ({
                    path: `$.properties.field${i}`,
                    type: 'string',
                  })),
                  references: req.input.text.includes('$ref')
                    ? [
                        {
                          target: 'https://example.org/reference.json',
                          path: '$.properties.address.$ref',
                        },
                      ]
                    : [],
                }
              : req.operation === 'limits'
                ? {
                    limits: {
                      attachments: {
                        applicable: {
                          maxNumber: 10,
                          maxSizeIndividualBytes: 1048576,
                          maxSizeAggregateBytes: 10485760,
                        },
                        platform: { maxNumber: 100 },
                        send: { maxNumber: 10 },
                        receive: { maxNumber: 20 },
                      },
                    },
                  }
                : {
                    destination: {
                      destinationId: req.input.destinationId,
                      status: 'inactive',
                      publicServices: [
                        {
                          identifier: 'audit-service',
                          regions: ['DE01'],
                          submissionSchemas: Array.from({ length: 23 }, (_, i) => ({
                            schemaUri: `https://example.org/schema-${i}.json`,
                            mimeType: 'application/json',
                          })),
                        },
                      ],
                    },
                    certificates: [
                      {
                        subject: 'CN=Public Audit',
                        issuer: 'CN=Public CA',
                        sha256: 'public-hash',
                        chainIndex: 0,
                        role: 'Teilnehmerzertifikat',
                        pem: '-----BEGIN CERTIFICATE-----\npublic-certificate\n-----END CERTIFICATE-----',
                        x5c: 'public-certificate',
                      },
                    ],
                    pvogRelations: [
                      {
                        uebergeordnetesObjektID: { value: 'parent' },
                        zustaendigkeit: [
                          { id: { value: 'context' }, rolle: [{ code: '02', name: 'Zuständig' }] },
                        ],
                      },
                    ],
                    pvogOnlineServices: [
                      {
                        id: 'service-id',
                        sourceIndex: 0,
                        matchesParent: true,
                        context: '$.pvogRelations[0]',
                        source: 'https://example.org/source',
                      },
                    ],
                    pvogOnlineServiceSources: [
                      {
                        data: {
                          bezeichnung: [
                            { languageCode: 'de', value: 'Öffentlicher Audit-Onlinedienst' },
                          ],
                          parameterIndividuell: [
                            {
                              parameterName: 'contact',
                              parameterWert: 'public',
                              languageCode: ['de'],
                            },
                          ],
                        },
                      },
                    ],
                  };
          return {
            ok: true,
            report: {
              protocolVersion: 1,
              appVersion: '0.1.0',
              createdAt: '2026-10-31T12:00:00Z',
              operation: req.operation,
              environment: req.environment,
              apiVersion: req.apiVersion,
              findings:
                req.operation === 'schema'
                  ? [f('confirmed', 'Schema eingelesen', '250 Strukturknoten')]
                  : req.operation === 'limits'
                    ? [f('confirmed', 'Größenlimits veröffentlicht', 'Paar geprüft')]
                    : [
                        f('confirmed', 'Basisprüfung', 'Öffentliche Konfiguration'),
                        f('error', 'Wichtige Abweichung', 'Konfiguration korrigieren'),
                        f('unknown', 'Offener Nachweis', 'Unabhängigen Nachweis ergänzen'),
                      ],
              data,
              requests: [],
              rawResponses: { fixture: 'raw-marker' },
            },
          };
        },
      },
    });
  });
  await page.goto('/');
});

test('F01 restores each scroll position and opens a new area at its beginning', async ({
  page,
}) => {
  await go(page, 'Hilfe');
  await page
    .getByRole('combobox', { name: 'Hilfethema', exact: true })
    .selectOption({ label: 'Fachdatenschemata lesen' });
  await page.getByText('Grundlagen und Grenzen', { exact: true }).click();
  await page.getByText('Beispiel anzeigen', { exact: true }).click();
  await page.locator('main').evaluate((e) => (e.scrollTop = e.scrollHeight));
  const before = await page.locator('main').evaluate((e) => e.scrollTop);
  expect(before).toBeGreaterThan(0);
  await go(page, 'Routing prüfen');
  await expect.poll(() => page.locator('main').evaluate((e) => e.scrollTop)).toBe(0);
  await expect(page.getByRole('textbox', { name: 'LeiKa-Schlüssel', exact: true })).toBeVisible();
  await go(page, 'Hilfe');
  await expect(page.getByRole('combobox', { name: 'Hilfethema', exact: true })).toHaveValue('6');
  await expect.poll(() => page.locator('main').evaluate((e) => e.scrollTop)).toBe(before);
});

test('F02 following a reference retains the original schema, report and structure filter', async ({
  page,
}) => {
  await go(page, 'Fachdatenschemata');
  const text = '{"type":"object","$ref":"https://example.org/reference.json"}';
  await page.getByRole('textbox', { name: 'JSON Schema / XSD', exact: true }).fill(text);
  await run(page);
  await page.getByRole('searchbox', { name: 'Struktur durchsuchen', exact: true }).fill('field249');
  await page.getByRole('button', { name: 'Referenz vorbereiten', exact: true }).click();
  await expect(
    page.getByRole('textbox', { name: 'Öffentliche HTTPS-URL', exact: true }),
  ).toHaveValue('https://example.org/reference.json');
  await page.getByRole('button', { name: 'Vorheriges Schema', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'JSON Schema / XSD', exact: true })).toHaveValue(
    text,
  );
  await expect(
    page.getByRole('searchbox', { name: 'Struktur durchsuchen', exact: true }),
  ).toHaveValue('field249');
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Nächstes Schema', exact: true }).click();
  await expect(
    page.getByRole('textbox', { name: 'Öffentliche HTTPS-URL', exact: true }),
  ).toHaveValue('https://example.org/reference.json');
});

test('F03 important findings precede source data and confirmed checks remain available', async ({
  page,
}) => {
  await go(page, 'Destination prüfen');
  await page.getByRole('textbox', { name: 'Destination-ID', exact: true }).fill(from);
  await run(page);
  const important = page.getByRole('heading', { name: 'Wichtige Abweichung', exact: true });
  const data = page.getByRole('heading', { name: 'Zertifikatsketten', exact: true });
  expect((await important.boundingBox())!.y).toBeLessThan((await data.boundingBox())!.y);
  const result = await page.locator('.results').boundingBox();
  const main = await page.locator('main').boundingBox();
  expect(result!.y).toBeGreaterThanOrEqual(main!.y - 1);
  expect(result!.y).toBeLessThan(400);
  await expect(page.getByRole('heading', { name: 'Basisprüfung', exact: true })).toBeHidden();
  await page.getByRole('button', { name: '1 Bestätigt', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Basisprüfung', exact: true })).toBeVisible();
  await expect(important).toBeHidden();
});

test('F04 input modes retain drafts but submit only the selected source', async ({ page }) => {
  await go(page, 'Fachdatenschemata');
  await page
    .getByRole('textbox', { name: 'JSON Schema / XSD', exact: true })
    .fill('{"type":"object"}');
  await page.getByRole('button', { name: 'HTTPS-URL', exact: true }).click();
  await page
    .getByRole('textbox', { name: 'Öffentliche HTTPS-URL', exact: true })
    .fill('https://example.org/schema.json');
  await run(page);
  const input = await page.evaluate(() => (window as any).inspectUx.calls.at(-1).request.input);
  expect(input.url).toBe('https://example.org/schema.json');
  expect(input.text).toBe('');
  await page.getByRole('button', { name: 'Einfügen', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'JSON Schema / XSD', exact: true })).toHaveValue(
    '{"type":"object"}',
  );
});

test('file mode submits the loaded file even after editing the text alternative', async ({
  page,
}) => {
  await go(page, 'Fachdatenschemata');
  await page.getByRole('button', { name: 'Datei', exact: true }).click();
  await page.getByLabel('Datei auswählen', { exact: true }).setInputFiles({
    name: 'public-schema.json',
    mimeType: 'application/json',
    buffer: Buffer.from('{"type":"number"}'),
  });
  await expect(page.getByText('Datei eingelesen:', { exact: false })).toContainText(
    'public-schema.json',
  );
  await page.getByRole('button', { name: 'Einfügen', exact: true }).click();
  await page
    .getByRole('textbox', { name: 'JSON Schema / XSD', exact: true })
    .fill('{"type":"boolean"}');
  await page.getByRole('button', { name: 'Datei', exact: true }).click();
  await run(page);
  expect(await page.evaluate(() => (window as any).inspectUx.calls.at(-1).request.input.text)).toBe(
    '{"type":"number"}',
  );
});

test('F05 PVOG requires ARS before making a routing request', async ({ page }) => {
  await go(page, 'Routing prüfen');
  await page.getByRole('textbox', { name: 'LeiKa-Schlüssel', exact: true }).fill('99150085001000');
  await page.getByRole('combobox', { name: 'Gebietsart', exact: true }).selectOption('areaId');
  await page.getByRole('textbox', { name: 'Gebietsschlüssel', exact: true }).fill('44273');
  await page.getByRole('checkbox', { name: 'PVOG-Adressierung mit abrufen', exact: true }).check();
  await page
    .getByRole('textbox', { name: 'PVOG-Leistungsbeschreibungs-ID', exact: true })
    .fill('L100012.LB.9413106');
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('12-stelligen ARS');
  expect(await page.evaluate(() => (window as any).inspectUx.calls.length)).toBe(0);
  await page.getByRole('combobox', { name: 'Gebietsart', exact: true }).selectOption('ars');
  await page.getByRole('textbox', { name: 'Gebietsschlüssel', exact: true }).fill('010010000000');
  await run(page);
});

test('F06 independent limits need only two destinations and retain the process workspace', async ({
  page,
}) => {
  await go(page, 'Prozessnachricht prüfen');
  await page
    .getByRole('textbox', { name: 'Prozessmodell-ID', exact: true })
    .fill('urn:audit:process');
  await page.getByRole('textbox', { name: 'Nachrichten-ID', exact: true }).fill('message');
  await page.getByRole('textbox', { name: 'Region', exact: true }).fill('DE01');
  await run(page);
  await page.getByRole('button', { name: 'Limits prüfen', exact: true }).click();
  await page.getByRole('textbox', { name: 'Absender-Destination', exact: true }).fill(from);
  await page.getByRole('textbox', { name: 'Empfänger-Destination', exact: true }).fill(to);
  await page.getByRole('button', { name: 'Limits abrufen', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Größenlimits für Anlagen', exact: true }),
  ).toBeVisible();
  expect(await page.evaluate(() => (window as any).inspectUx.calls.at(-1).request.operation)).toBe(
    'limits',
  );
  await page.getByRole('button', { name: 'Prozessnachricht suchen', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'Prozessmodell-ID', exact: true })).toHaveValue(
    'urn:audit:process',
  );
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toBeVisible();
});

test('F07 structure search reaches a field beyond the first hundred without extra requests', async ({
  page,
}) => {
  await go(page, 'Fachdatenschemata');
  await page.getByRole('textbox', { name: 'JSON Schema / XSD', exact: true }).fill('{}');
  await run(page);
  expect(
    await page
      .getByRole('region', { name: 'Strukturtreffer', exact: true })
      .locator('tbody tr')
      .count(),
  ).toBe(100);
  await page.getByRole('searchbox', { name: 'Struktur durchsuchen', exact: true }).fill('field249');
  await expect(
    page.getByRole('region', { name: 'Strukturtreffer', exact: true }).locator('tbody tr'),
  ).toHaveCount(1);
  await expect(page.getByRole('region', { name: 'Strukturtreffer', exact: true })).toContainText(
    'field249',
  );
  expect(await page.evaluate(() => (window as any).inspectUx.calls.length)).toBe(1);
  await page
    .getByRole('textbox', { name: 'JSON Schema / XSD', exact: true })
    .fill('{"type":"object"}');
  await run(page);
  await expect(
    page.getByRole('searchbox', { name: 'Struktur durchsuchen', exact: true }),
  ).toHaveValue('');
});

test('F08 schema search filters individual rows and can explicitly show all references', async ({
  page,
}) => {
  await go(page, 'Destination prüfen');
  await page.getByRole('textbox', { name: 'Destination-ID', exact: true }).fill(from);
  await run(page);
  await page
    .getByRole('searchbox', { name: 'Leistung oder Schema suchen', exact: true })
    .fill('schema-22');
  await page.locator('.service > summary').click();
  await expect(page.locator('.schema-line')).toHaveCount(1);
  await expect(page.locator('.schema-line')).toContainText('schema-22');
  await page.getByRole('button', { name: 'Alle 23 Schemas anzeigen', exact: true }).click();
  await expect(page.locator('.schema-line')).toHaveCount(23);
});

test('F09 HTML preview renders the formatted inert report while JSON shows data', async ({
  page,
}) => {
  await go(page, 'Destination prüfen');
  await page.getByRole('textbox', { name: 'Destination-ID', exact: true }).fill(from);
  await run(page);
  await page.getByRole('button', { name: 'Bericht exportieren', exact: true }).click();
  const dialog = page.getByRole('dialog');
  const frame = dialog.frameLocator('iframe');
  await expect(frame.getByRole('heading', { name: 'FBForge Inspect', exact: true })).toBeVisible();
  await expect(dialog.locator('iframe')).toHaveAttribute('sandbox', 'allow-same-origin');
  await expect(frame.locator('body')).not.toContainText('raw-marker');
  await dialog.getByRole('combobox', { name: 'Format', exact: true }).selectOption('json');
  await expect(dialog.locator('pre')).toContainText('"operation"');
  await dialog.getByRole('checkbox', { name: 'Rohantworten einschließen', exact: true }).check();
  await expect(dialog.locator('pre')).toContainText('raw-marker');
  await dialog.getByRole('combobox', { name: 'Format', exact: true }).selectOption('html');
  await dialog
    .frameLocator('iframe')
    .getByText('Beleg und Feldpfad', { exact: true })
    .first()
    .focus();
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  await expect(
    page.getByRole('button', { name: 'Bericht exportieren', exact: true }),
  ).toBeFocused();
});

test('F10 certificate and context details use full table width and published names are visible', async ({
  page,
}) => {
  await go(page, 'Destination prüfen');
  await page.getByRole('textbox', { name: 'Destination-ID', exact: true }).fill(from);
  await run(page);
  await page.getByText('Zertifikat und x5c anzeigen', { exact: true }).click();
  await expect(
    page.locator('td[colspan="3"]').filter({ hasText: 'BEGIN CERTIFICATE' }),
  ).toHaveCount(1);
  await page.getByText('Alle Kontextfelder und Feldpfad', { exact: true }).click();
  await expect(
    page.locator('td[colspan="3"]').filter({ hasText: '$.pvogRelations[0].zustaendigkeit[0]' }),
  ).toHaveCount(1);
  await expect(
    page.getByRole('heading', { name: 'Öffentlicher Audit-Onlinedienst', exact: true }),
  ).toBeVisible();
});

test('F11 unknown findings do not repeat generic next steps and confirmed labels describe observations', async ({
  page,
}) => {
  await go(page, 'Destination prüfen');
  await page.getByRole('textbox', { name: 'Destination-ID', exact: true }).fill(from);
  await run(page);
  await expect(
    page.getByText('Quelle oder Konfiguration gezielt prüfen.', { exact: true }),
  ).toHaveCount(0);
});

test('F12 help starts with steps, preserves background information and offers working references', async ({
  page,
}) => {
  await go(page, 'Hilfe');
  await page
    .getByRole('combobox', { name: 'Hilfethema', exact: true })
    .selectOption({ label: 'Routing und PVOG' });
  await expect(page.locator('.help-topic ol li')).toHaveCount(4);
  await expect(page.getByText('Grundlagen und Grenzen', { exact: true })).toBeVisible();
  await expect(page.locator('.help-topic details')).not.toHaveAttribute('open', '');
  await page.getByText('Grundlagen und Grenzen', { exact: true }).click();
  await expect(page.locator('.help-topic details')).toHaveAttribute('open', '');
  await page.getByRole('link', { name: 'FIT-Connect-Dokumentation', exact: true }).click();
  expect(await page.evaluate(() => (window as any).inspectUx.calls.at(-1))).toMatchObject({
    command: 'open_reference',
    reference: 'fitko',
  });
});
