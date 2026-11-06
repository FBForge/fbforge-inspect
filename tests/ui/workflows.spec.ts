import { selectAppearance } from './theme';
import { test, expect } from './fixtures';
const uuid = '00000000-0000-4000-8000-000000000001';
async function navigate(page: import('@playwright/test').Page, name: string) {
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name, exact: true })
    .click();
}
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const state = {
      calls: [] as unknown[],
      saveOutcomes: [] as (boolean | string)[],
      diagnosisData: null as Record<string, unknown> | null,
    };
    Object.assign(window, {
      inspectTest: state,
      isTauri: true,
      __TAURI_INTERNALS__: {
        invoke: async (command: string, args: Record<string, any>) => {
          state.calls.push({ command, args });
          if (command === 'save_report') {
            const outcome = state.saveOutcomes.shift() ?? true;
            if (outcome === 'failure') throw new Error('synthetic write failure');
            return outcome;
          }
          if (command === 'cancel' || command === 'open_reference') return;
          const request = args.request;
          return {
            ok: true,
            report: {
              protocolVersion: 1,
              appVersion: '0.1.0',
              createdAt: '2026-10-28T12:00:00Z',
              operation: request.operation,
              environment: request.environment,
              apiVersion: request.apiVersion,
              findings: [
                {
                  status: 'confirmed',
                  title: 'Destination-Status',
                  detail: 'active',
                  path: '$.status',
                  source: 'https://example.org/fixture',
                  nextStep: '',
                  limitation: '',
                },
              ],
              requests: [],
              data: state.diagnosisData ?? {
                destination: {
                  destinationId: request.input.destinationId,
                  status: 'active',
                  type: 'A',
                  metadataVersions: ['1.2.0'],
                  publicServices: [],
                },
              },
              rawResponses: { 'https://example.org': 'raw-fixture-only' },
            },
          };
        },
      },
    });
  });
  await page.goto('/');
});
test('PVOG suggestions need no manual ID and selection preserves the routing context', async ({
  page,
}) => {
  await navigate(page, 'Routing prüfen');
  await page.getByLabel('LeiKa-Schlüssel', { exact: true }).fill('99008001014002');
  await page.getByLabel('Gebietsschlüssel', { exact: true }).fill('020000000000');
  await page.getByRole('checkbox', { name: 'PVOG-Adressierung mit abrufen', exact: true }).check();
  await page.evaluate(() => {
    (window as any).inspectTest.diagnosisData = {
      routes: [],
      pvogCandidates: [
        {
          id: 'L1.LB.service',
          name: 'Fiktive Leistung <img src=x onerror=alert(1)>',
          source: 'https://example.org/servicedescriptions/leikaid',
          ars: ['020000000000'],
          organisationIds: ['L1.OE.office'],
        },
      ],
    };
  });
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Leistungsbeschreibung für den PVOG-Abgleich wählen' }),
  ).toBeVisible();
  await expect(
    page.getByRole('heading', { name: 'Fiktive Leistung <img src=x onerror=alert(1)>' }),
  ).toBeVisible();
  await expect(page.locator('.results img')).toHaveCount(0);
  await page.getByText('Quelle und Organisationseinheiten', { exact: true }).click();
  await expect(page.getByText('OE: L1.OE.office', { exact: true })).toBeVisible();
  await page.evaluate(() => {
    (window as any).inspectTest.diagnosisData = { routes: [], pvogRelations: [] };
  });
  await page
    .getByRole('button', { name: 'Mit dieser Leistungsbeschreibung prüfen', exact: true })
    .click();
  const calls = await page.evaluate(() =>
    (window as any).inspectTest.calls.filter((call: any) => call.command === 'diagnose'),
  );
  expect(calls).toHaveLength(2);
  expect(calls[0].args.request.input.lbId).toBe('');
  expect(calls[1].args.request).toMatchObject({
    operation: 'routing',
    environment: 'TEST',
    input: {
      service: '99008001014002',
      region: '020000000000',
      regionKind: 'ars',
      pvog: true,
      lbId: 'L1.LB.service',
    },
  });
  await navigate(page, 'Destination prüfen');
  await navigate(page, 'Routing prüfen');
  await expect(page.getByLabel('PVOG-Leistungsbeschreibungs-ID', { exact: true })).toHaveValue(
    'L1.LB.service',
  );
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toBeVisible();
});
test('network configuration is reachable from Start and Info', async ({ page }) => {
  await page.getByRole('button', { name: 'Netzwerk und Verbindungsprobleme', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Netzwerk', exact: true })).toBeVisible();
  await expect(page.getByRole('combobox', { name: 'Verbindung', exact: true })).toHaveValue(
    'direct',
  );
  await navigate(page, 'Info');
  await page.getByRole('button', { name: 'Netzwerk öffnen', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Netzwerk', exact: true })).toBeVisible();
});
test('navigation retains independent drafts and reports across pages, environments and versions', async ({
  page,
}) => {
  await navigate(page, 'Destination prüfen');
  await page.getByRole('textbox', { name: 'Destination-ID', exact: true }).fill(uuid);
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toBeVisible();
  await expect(
    page.getByRole('heading', { name: 'Bereit für Ihre Prüfung', exact: true }),
  ).toHaveCount(0);
  await navigate(page, 'Routing prüfen');
  await page.getByRole('textbox', { name: 'LeiKa-Schlüssel', exact: true }).fill('99150085001000');
  await page.getByRole('textbox', { name: 'Gebietsschlüssel', exact: true }).fill('010010000000');
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await navigate(page, 'Hilfe');
  await navigate(page, 'Destination prüfen');
  await expect(page.getByRole('textbox', { name: 'Destination-ID', exact: true })).toHaveValue(
    uuid,
  );
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toBeVisible();
  await page.getByRole('combobox', { name: 'Umgebung', exact: true }).selectOption('PROD');
  await expect(page.getByRole('textbox', { name: 'Destination-ID', exact: true })).toHaveValue('');
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toHaveCount(0);
  await page.getByRole('combobox', { name: 'Umgebung', exact: true }).selectOption('TEST');
  await expect(page.getByRole('textbox', { name: 'Destination-ID', exact: true })).toHaveValue(
    uuid,
  );
  await page.getByRole('combobox', { name: 'API', exact: true }).selectOption('v2');
  await expect(page.getByRole('textbox', { name: 'Destination-ID', exact: true })).toHaveValue('');
  await page.getByRole('combobox', { name: 'API', exact: true }).selectOption('v3');
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toBeVisible();
  await navigate(page, 'Routing prüfen');
  await expect(page.getByRole('textbox', { name: 'LeiKa-Schlüssel', exact: true })).toHaveValue(
    '99150085001000',
  );
  await expect(page.getByRole('textbox', { name: 'Gebietsschlüssel', exact: true })).toHaveValue(
    '010010000000',
  );
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toBeVisible();
  expect(
    await page.evaluate(
      () => (window as any).inspectTest.calls.filter((c: any) => c.command === 'diagnose').length,
    ),
  ).toBe(2);
  await page.reload();
  await navigate(page, 'Destination prüfen');
  await expect(page.getByRole('textbox', { name: 'Destination-ID', exact: true })).toHaveValue('');
});
test('many schemas stay collapsed and searchable, and preparing one preserves the destination', async ({
  page,
}) => {
  await page.evaluate(() => {
    (window as any).inspectTest.diagnosisData = {
      destination: {
        destinationId: 'fixture',
        publicServices: [
          {
            identifier: 'service-context',
            regions: ['DE01'],
            submissionSchemas: Array.from({ length: 23 }, (_, i) => ({
              schemaUri: `https://example.org/schema-${i}.json`,
              mimeType: 'application/json',
            })),
          },
        ],
      },
    };
  });
  await navigate(page, 'Destination prüfen');
  await page.getByRole('textbox', { name: 'Destination-ID', exact: true }).fill(uuid);
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await expect(page.getByText('23 Schemas', { exact: true })).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Schema ansehen', exact: true }).first(),
  ).not.toBeVisible();
  await page
    .getByRole('searchbox', { name: 'Leistung oder Schema suchen', exact: true })
    .fill('schema-22');
  await page.locator('.service > summary').click();
  await page.getByRole('button', { name: 'Schema ansehen', exact: true }).last().click();
  await expect(
    page.getByRole('textbox', { name: 'Öffentliche HTTPS-URL', exact: true }),
  ).toHaveValue('https://example.org/schema-22.json');
  await navigate(page, 'Destination prüfen');
  await expect(page.getByRole('textbox', { name: 'Destination-ID', exact: true })).toHaveValue(
    uuid,
  );
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toBeVisible();
  await expect(
    page.getByRole('searchbox', { name: 'Leistung oder Schema suchen', exact: true }),
  ).toHaveValue('schema-22');
  await expect(page.locator('.service')).toHaveAttribute('open', '');
  expect(
    await page.evaluate(
      () => (window as any).inspectTest.calls.filter((c: any) => c.command === 'diagnose').length,
    ),
  ).toBe(1);
});
test('public JWK and x5c certificate representations are available without raw responses', async ({
  page,
}) => {
  await page.evaluate(() => {
    (window as any).inspectTest.diagnosisData = {
      keys: [
        {
          kid: 'public-key',
          algorithm: 'RSA-OAEP-256',
          bits: 3072,
          x5cCount: 1,
          publicJwk: {
            kty: 'RSA',
            key_ops: ['wrapKey'],
            alg: 'RSA-OAEP-256',
            x5c: ['public-base64-certificate'],
          },
        },
      ],
      certificates: [
        {
          kid: 'public-key',
          chainIndex: 0,
          role: 'Teilnehmerzertifikat',
          subject: 'CN=Public Fixture',
          issuer: 'CN=Public Fixture',
          sha256: 'public-digest',
          pem: '-----BEGIN CERTIFICATE-----\npublic-base64-certificate\n-----END CERTIFICATE-----',
          x5c: 'public-base64-certificate',
        },
      ],
    };
  });
  await navigate(page, 'Schlüssel prüfen');
  await page
    .getByRole('textbox', { name: 'Öffentliches JWK / JWKS', exact: true })
    .fill('{"kty":"RSA"}');
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await page.getByText('JWK-Felder einschließlich x5c anzeigen', { exact: true }).click();
  await expect(page.locator('.key-card pre')).toContainText('"key_ops"');
  await expect(page.locator('.key-card pre')).toContainText('"x5c"');
  await page.getByText('Zertifikat und x5c anzeigen', { exact: true }).click();
  await expect(page.getByText('Key Usage:', { exact: false })).toHaveCount(0);
  await expect(
    page.locator('table pre').filter({ hasText: '-----BEGIN CERTIFICATE-----' }),
  ).toBeVisible();
});
test('PVOG retains roles, source contexts and parameter languages without assigning a foreign response', async ({
  page,
}) => {
  await page.evaluate(() => {
    const relation = {
      uebergeordnetesObjektID: { value: 'od', schemeAgencyID: 'L1', schemeID: 'source-scheme' },
      uebergeordnetesObjektKlasse: { code: '003', name: 'Onlinedienst' },
      zustaendigkeit: [
        {
          id: { value: 'relation' },
          rolle: [
            { code: '02', name: 'Zuständige Stelle' },
            { code: '03', name: 'Ansprechpunkt' },
          ],
          leistungID: [{ value: 'lb', schemeAgencyID: 'L1' }],
          gebietID: [{ value: '010000000000', schemeID: 'ars' }],
          kommunikationssystem: [{ kanal: { code: '004' }, kennung: 'unverified-destination' }],
          gueltigkeit: [{ beginn: '2026-01-01', ende: '2026-12-31' }],
        },
      ],
    };
    (window as any).inspectTest.diagnosisData = {
      pvogRelations: [relation, relation],
      pvogOnlineServices: [
        {
          id: 'L1.OD.od',
          context: '$.pvogRelations[0]',
          matchesParent: true,
          source: 'https://example.org/source',
          sourceIndex: 0,
        },
        {
          id: 'L1.OD.foreign',
          context: '$.pvogRelations[1]',
          matchesParent: false,
          sourceIndex: 1,
        },
      ],
      pvogOnlineServiceSources: [
        {
          data: {
            parameterEfA: [
              {
                parameterEfA: { code: 'efa.contact.url' },
                parameterWert: '<img src="https://example.org/not-loaded">',
                languageCode: ['de', 'en'],
              },
            ],
            parameterIndividuell: [{ parameterName: 'missing-value' }],
          },
        },
        {
          data: {
            parameterIndividuell: [
              { parameterName: 'foreign-parameter', parameterWert: 'wrong-context' },
            ],
          },
        },
      ],
    };
  });
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Routing prüfen', exact: true })
    .click();
  await page.getByRole('textbox', { name: 'LeiKa-Schlüssel', exact: true }).fill('99150085001000');
  await page.getByRole('textbox', { name: 'Gebietsschlüssel', exact: true }).fill('010000000000');
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await page.getByText('PVOG-Zuständigkeitskontexte im Detail', { exact: true }).click();
  await page.getByText('PVOG-Onlinedienste und Parameter im Detail', { exact: true }).click();
  await expect(
    page.getByRole('cell', { name: /02 · Zuständige Stelle, 03 · Ansprechpunkt/ }),
  ).toHaveCount(2);
  await expect(page.getByRole('cell', { name: /efa.contact.url/ })).toBeVisible();
  await expect(page.getByRole('cell', { name: /de, en/ })).toBeVisible();
  await expect(
    page.getByRole('cell', { name: 'Kein Wert veröffentlicht', exact: true }),
  ).toBeVisible();
  await expect(page.getByRole('cell', { name: /foreign-parameter/ })).toHaveCount(0);
  await expect(page.locator('img[src="https://example.org/not-loaded"]')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'unverified-destination' })).toHaveCount(0);
  await page.screenshot({ path: 'test-results/pvog-contexts-light.png', fullPage: true });
  await selectAppearance(page, 'dark');
  await page.setViewportSize({ width: 700, height: 650 });
  expect(
    await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth),
  ).toBeTruthy();
  await page.screenshot({ path: 'test-results/pvog-contexts-700-dark.png', fullPage: true });
  await page.getByText('Alle Kontextfelder und Feldpfad', { exact: true }).first().click();
  await expect(
    page
      .getByRole('region', { name: 'PVOG-Zuständigkeitskontexte', exact: true })
      .getByText('$.pvogRelations[0].zustaendigkeit[0]', { exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Bericht exportieren' }).click();
  await page
    .getByRole('dialog')
    .getByRole('combobox', { name: 'Format', exact: true })
    .selectOption('json');
  await expect(page.getByRole('dialog').locator('pre')).toContainText('kommunikationssystem');
  await expect(page.getByRole('dialog').locator('pre')).toContainText('languageCode');
});
test('destination diagnosis and export preview do not include raw responses by default', async ({
  page,
}) => {
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Destination prüfen', exact: true })
    .click();
  await page.getByRole('textbox', { name: 'Destination-ID', exact: true }).fill(uuid);
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Bericht exportieren' }).click();
  await page
    .getByRole('dialog')
    .getByRole('combobox', { name: 'Format', exact: true })
    .selectOption('json');
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  await expect(dialog.locator('pre')).not.toContainText('raw-fixture-only');
  await dialog.getByRole('checkbox', { name: 'Rohantworten einschließen' }).check();
  await expect(dialog.locator('pre')).toContainText('raw-fixture-only');
  await page.keyboard.press('Escape');
  await expect(dialog).not.toBeVisible();
  await page.getByRole('button', { name: 'Bericht exportieren' }).click();
  await page
    .getByRole('dialog')
    .getByRole('combobox', { name: 'Format', exact: true })
    .selectOption('json');
  await expect(
    page.getByRole('dialog').getByRole('checkbox', { name: 'Rohantworten einschließen' }),
  ).not.toBeChecked();
  await page.keyboard.press('Escape');
  await page.getByRole('textbox', { name: 'Destination-ID', exact: true }).fill('changed');
  await expect(
    page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true }),
  ).not.toBeVisible();
});
test('v2 disables process search and STAGE identifies PROD routing', async ({ page }) => {
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Prozessnachricht prüfen', exact: true })
    .click();
  await page.getByRole('combobox', { name: 'API', exact: true }).selectOption('v2');
  await expect(page.getByRole('button', { name: 'Prüfung starten', exact: true })).toBeDisabled();
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Routing prüfen', exact: true })
    .click();
  await page.getByRole('combobox', { name: 'Umgebung', exact: true }).selectOption('STAGE');
  await expect(
    page.getByText('STAGE hat keine eigene Routing-API.', { exact: false }),
  ).toBeVisible();
});
test('private JWK is rejected before invocation', async ({ page }) => {
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Schlüssel prüfen', exact: true })
    .click();
  await page
    .getByRole('textbox', { name: 'Öffentliches JWK / JWKS' })
    .fill('{"kty":"RSA","d":"synthetic-private-marker"}');
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Private oder symmetrische Schlüssel');
  expect(
    await page.evaluate(() => JSON.stringify((window as any).inspectTest.calls)),
  ).not.toContain('synthetic-private-marker');
});
test('both themes have no horizontal overflow and navigation is keyboard reachable', async ({
  page,
}) => {
  await selectAppearance(page, 'dark');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.screenshot({ path: 'test-results/start-dark.png' });
  await page.setViewportSize({ width: 700, height: 650 });
  expect(
    await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth),
  ).toBeTruthy();
  await page.keyboard.press('Tab');
  await expect(page.locator(':focus')).toHaveCount(1);
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Hilfe', exact: true })
    .click();
  await expect(page.getByRole('heading', { name: 'Anleitung auf diesem Gerät' })).toBeVisible();
  await page.getByRole('button', { name: 'Bebilderte Anleitung im Browser' }).click();
  expect(await page.evaluate(() => (window as any).inspectTest.calls.at(-1).command)).toBe(
    'open_reference',
  );
});

test('export cancellation keeps the preview and write failures are announced inside the dialog', async ({
  page,
}) => {
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Destination prüfen', exact: true })
    .click();
  await page.getByRole('textbox', { name: 'Destination-ID', exact: true }).fill(uuid);
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await page.getByRole('button', { name: 'Bericht exportieren' }).click();
  await page
    .getByRole('dialog')
    .getByRole('combobox', { name: 'Format', exact: true })
    .selectOption('json');
  const dialog = page.getByRole('dialog');
  await dialog.getByRole('combobox', { name: 'Format', exact: true }).selectOption('json');
  await page.evaluate(() => {
    (window as any).inspectTest.saveOutcomes = [false, 'failure', true];
  });
  await dialog.getByRole('button', { name: 'Speicherort wählen', exact: true }).click();
  await expect(dialog).toBeVisible();
  await expect(page.getByRole('status')).not.toContainText(['Bericht lokal gespeichert']);
  await dialog.getByRole('button', { name: 'Speicherort wählen', exact: true }).click();
  await expect(dialog.getByRole('alert')).toContainText('Bericht konnte nicht gespeichert werden.');
  await dialog.getByRole('button', { name: 'Speicherort wählen', exact: true }).click();
  await expect(dialog).not.toBeVisible();
  const saves = await page.evaluate(() =>
    (window as any).inspectTest.calls.filter((call: any) => call.command === 'save_report'),
  );
  expect(saves).toHaveLength(3);
  for (const save of saves) {
    expect(save.args.format).toBe('json');
    expect(JSON.parse(save.args.content).rawResponses).toBeUndefined();
  }
  await expect(page.getByRole('button', { name: 'Bericht exportieren' })).toBeFocused();
});

test('closing the export preview returns keyboard focus to its trigger', async ({ page }) => {
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Destination prüfen', exact: true })
    .click();
  await page.getByRole('textbox', { name: 'Destination-ID', exact: true }).fill(uuid);
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  const trigger = page.getByRole('button', { name: 'Bericht exportieren' });
  await trigger.click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(trigger).toBeFocused();
});

test('routing and area results prepare their targets without losing the routing draft', async ({
  page,
}) => {
  await navigate(page, 'Routing prüfen');
  await page.getByLabel('LeiKa-Schlüssel', { exact: true }).fill('99008001014002');
  await page.getByLabel('Gebietsschlüssel', { exact: true }).fill('020000000000');
  await page.evaluate((id) => {
    (window as any).inspectTest.diagnosisData = {
      routes: [{ destinationId: id, destinationName: 'Fiktiver Zustellpunkt' }],
    };
  }, uuid);
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await page
    .getByRole('region', { name: 'Routing- und Suchergebnisse' })
    .getByRole('button', { name: 'Destination prüfen', exact: true })
    .click();
  await expect(page.getByLabel('Destination-ID', { exact: true })).toHaveValue(uuid);
  await expect(page.getByRole('heading', { name: 'Bereit für Ihre Prüfung' })).toBeVisible();
  await navigate(page, 'Routing prüfen');
  await expect(page.getByLabel('LeiKa-Schlüssel', { exact: true })).toHaveValue('99008001014002');
  await expect(page.getByRole('region', { name: 'Routing- und Suchergebnisse' })).toContainText(
    'Fiktiver Zustellpunkt',
  );
  await page.getByText('Gebiet über Ortsnamen finden', { exact: true }).click();
  await page.getByLabel('Ortsname', { exact: true }).fill('Musterstadt');
  await page.evaluate(() => {
    (window as any).inspectTest.diagnosisData = { areas: [{ id: '12345', name: 'Musterstadt' }] };
  });
  await page.getByRole('button', { name: 'Gebiete suchen', exact: true }).click();
  await page.getByRole('button', { name: 'Gebiet übernehmen', exact: true }).click();
  await expect(page.getByRole('combobox', { name: 'Gebietsart', exact: true })).toHaveValue(
    'areaId',
  );
  await expect(page.getByLabel('Gebietsschlüssel', { exact: true })).toHaveValue('12345');
});

test('cancelling ignores a late successful result and keeps the entered destination', async ({
  page,
}) => {
  await page.evaluate(() => {
    const previous = (window as any).__TAURI_INTERNALS__.invoke;
    let finish: (value: unknown) => void;
    (window as any).__TAURI_INTERNALS__.invoke = async (command: string, args: any) => {
      if (command === 'diagnose') {
        const response = await previous(command, args);
        return new Promise((resolve) => {
          finish = () => resolve(response);
        });
      }
      if (command === 'cancel') {
        finish(null);
        return;
      }
      return previous(command, args);
    };
  });
  await navigate(page, 'Destination prüfen');
  await page.getByLabel('Destination-ID', { exact: true }).fill(uuid);
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Abbrechen', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Prüfung starten', exact: true })).toBeEnabled();
  await expect(page.getByRole('status').filter({ hasText: 'Prüfung abgebrochen' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toHaveCount(0);
  await expect(page.getByLabel('Destination-ID', { exact: true })).toHaveValue(uuid);
});

test('failed cancellation is announced and a subsequent completion is still accepted', async ({
  page,
}) => {
  await page.evaluate(() => {
    const previous = (window as any).__TAURI_INTERNALS__.invoke;
    (window as any).__TAURI_INTERNALS__.invoke = async (command: string, args: any) => {
      if (command === 'diagnose') {
        const response = await previous(command, args);
        return new Promise((resolve) => {
          (window as any).finishDiagnosis = () => resolve(response);
        });
      }
      if (command === 'cancel') throw new Error('synthetic cancellation failure');
      return previous(command, args);
    };
  });
  await navigate(page, 'Destination prüfen');
  await page.getByLabel('Destination-ID', { exact: true }).fill(uuid);
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await page.getByRole('button', { name: 'Abbrechen', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Prüfung konnte nicht abgebrochen werden.');
  await page.evaluate(() => (window as any).finishDiagnosis());
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toBeVisible();
  await expect(page.getByRole('alert')).toHaveCount(0);
});

test('manual proxy stays in memory, applies across tasks and is excluded from report exports', async ({
  page,
}) => {
  await navigate(page, 'Netzwerk');
  await page.getByRole('combobox', { name: 'Verbindung', exact: true }).selectOption('manual');
  await page.getByLabel('Proxy-Adresse', { exact: true }).fill('http://proxy.example:8080');
  await page
    .getByLabel('Proxy-Benutzername (optional)', { exact: true })
    .fill('fixture-proxy-user');
  await page.getByLabel('Proxy-Passwort', { exact: true }).fill('fixture-proxy-password');
  await expect(page.getByText('Bei http://', { exact: false })).toBeVisible();
  await page.evaluate(() => {
    (window as any).inspectTest.diagnosisData = {
      networkChecks: [
        {
          name: 'FIT-Connect',
          host: 'test.fit-connect.fitko.dev',
          port: 443,
          method: 'GET',
          url: 'https://test.fit-connect.fitko.dev/.well-known/jwks.json',
          status: 'confirmed',
          httpStatus: 200,
          durationMs: 123,
          detail: 'Verbindung erfolgreich.',
        },
      ],
    };
  });
  await page.getByRole('button', { name: 'Verbindungen prüfen', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Netzwerkverbindungen', exact: true }),
  ).toBeVisible();
  await page.getByText('Freigabeliste für die IT anzeigen', { exact: true }).click();
  await expect(
    page.getByText('Ziel: test.fit-connect.fitko.dev:443', { exact: false }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Bericht exportieren', exact: true }).click();
  await page.getByRole('combobox', { name: 'Format', exact: true }).selectOption('json');
  const content = await page.locator('dialog pre').innerText();
  expect(content).toContain('networkChecks');
  expect(content).not.toContain('fixture-proxy-user');
  expect(content).not.toContain('fixture-proxy-password');
  await page.getByRole('button', { name: 'Schließen', exact: true }).click();
  await navigate(page, 'Destination prüfen');
  await page.getByLabel('Destination-ID', { exact: true }).fill(uuid);
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  const requests = await page.evaluate(() =>
    (window as any).inspectTest.calls
      .filter((c: any) => c.command === 'diagnose')
      .map((c: any) => c.args.request),
  );
  for (const request of requests)
    expect(request.network).toEqual({
      mode: 'manual',
      proxyUrl: 'http://proxy.example:8080',
      username: 'fixture-proxy-user',
      password: 'fixture-proxy-password',
    });
  await navigate(page, 'Netzwerk');
  await page.getByRole('combobox', { name: 'Verbindung', exact: true }).selectOption('direct');
  await expect(
    page.getByRole('heading', { name: 'Netzwerkverbindungen', exact: true }),
  ).toHaveCount(0);
  await page.getByRole('button', { name: 'Verbindungen prüfen', exact: true }).click();
  expect(
    await page.evaluate(
      () =>
        (window as any).inspectTest.calls.filter((c: any) => c.command === 'diagnose').at(-1).args
          .request.network,
    ),
  ).toEqual({ mode: 'direct' });
  const stored = await page.evaluate(() => JSON.stringify({ ...localStorage, ...sessionStorage }));
  expect(stored).not.toContain('fixture-proxy');
  await page.reload();
  await navigate(page, 'Netzwerk');
  await page.getByRole('combobox', { name: 'Verbindung', exact: true }).selectOption('manual');
  await expect(page.getByLabel('Proxy-Passwort', { exact: true })).toHaveValue('');
});

test('network checks use prior public certificates only from the current environment', async ({
  page,
}) => {
  await navigate(page, 'Schlüssel prüfen');
  await page.getByLabel('Öffentliches JWK / JWKS', { exact: true }).fill('{"kty":"RSA"}');
  await page.evaluate(() => {
    (window as any).inspectTest.diagnosisData = {
      certificates: [
        { pem: '-----BEGIN CERTIFICATE-----\nfixture-public\n-----END CERTIFICATE-----' },
      ],
    };
  });
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await navigate(page, 'Netzwerk');
  await expect(
    page.getByRole('checkbox', { name: 'Zertifikatsendpunkte', exact: false }),
  ).toBeEnabled();
  await page.getByRole('button', { name: 'Verbindungen prüfen', exact: true }).click();
  expect(
    await page.evaluate(
      () =>
        (window as any).inspectTest.calls.filter((c: any) => c.command === 'diagnose').at(-1).args
          .request.input.certificates,
    ),
  ).toHaveLength(1);
  await page.getByRole('combobox', { name: /^Umgebung/ }).selectOption('PROD');
  await expect(
    page.getByRole('checkbox', { name: 'Zertifikatsendpunkte', exact: false }),
  ).toBeDisabled();
  await page.getByRole('button', { name: 'Verbindungen prüfen', exact: true }).click();
  expect(
    await page.evaluate(
      () =>
        (window as any).inspectTest.calls.filter((c: any) => c.command === 'diagnose').at(-1).args
          .request.input.certificates,
    ),
  ).toEqual([]);
});

test('automatic PVOG comparison shows matches and differences without a redundant chooser', async ({
  page,
}) => {
  await navigate(page, 'Routing prüfen');
  await expect(
    page.getByRole('checkbox', { name: 'PVOG-Adressierung mit abrufen', exact: true }),
  ).toBeChecked();
  await page.getByLabel('LeiKa-Schlüssel', { exact: true }).fill('99008001014002');
  await page.getByLabel('Gebietsschlüssel', { exact: true }).fill('020000000000');
  await page.evaluate(() => {
    (window as any).inspectTest.diagnosisData = {
      routes: [{ destinationId: 'same', destinationName: 'Beispielbehörde' }],
      pvogRelations: [],
      pvogSelectedLbId: 'L1.LB.only',
      pvogAutomaticallySelected: true,
      pvogCandidates: [{ id: 'L1.LB.only' }],
      routingComparison: [
        {
          destinationId: 'same',
          routingPath: '$.routes[0]',
          status: 'confirmed',
          detail: 'Dieselbe Destination-ID in beiden Quellen.',
          pvogContexts: ['$.pvogRelations[0]'],
          routing: { destinationId: 'same', oeId: 'OE1' },
        },
        {
          destinationId: 'pvog-only',
          routingPath: '',
          status: 'warning',
          detail: 'Nur in PVOG enthalten.',
          pvogContexts: ['$.pvogRelations[1]'],
        },
      ],
    };
  });
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await expect(
    page
      .locator('.service-group p')
      .filter({ hasText: 'Dieselbe Destination-ID in beiden Quellen.' }),
  ).toBeVisible();
  await expect(
    page.locator('.service-group p').filter({ hasText: 'Nur in PVOG enthalten.' }),
  ).toBeVisible();
  await expect(
    page.getByText('Leistungsbeschreibung automatisch ausgewählt:', { exact: false }),
  ).toBeVisible();
  await expect(
    page.getByRole('heading', { name: 'Leistungsbeschreibung für den PVOG-Abgleich wählen' }),
  ).toHaveCount(0);
  await page.getByText('Quellkontexte anzeigen', { exact: true }).first().click();
  await expect(page.locator('.service-group pre')).toContainText('OE1');
});
