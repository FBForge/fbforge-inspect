import { selectAppearance } from './theme';
import { test, expect } from '@playwright/test';
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
          if (command === 'cancel') return;
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
    page.getByText('$.pvogRelations[0].zustaendigkeit[0]', { exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Bericht exportieren' }).click();
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
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  await expect(dialog.locator('pre')).not.toContainText('raw-fixture-only');
  await dialog.getByRole('checkbox', { name: 'Rohantworten einschließen' }).check();
  await expect(dialog.locator('pre')).toContainText('raw-fixture-only');
  await page.keyboard.press('Escape');
  await expect(dialog).not.toBeVisible();
  await page.getByRole('button', { name: 'Bericht exportieren' }).click();
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
  await expect(page.getByRole('heading', { name: 'Schnellstart' })).toBeVisible();
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
