import { test, expect } from '@playwright/test';
const uuid = '00000000-0000-4000-8000-000000000001';
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const state = { calls: [] as unknown[], saveOutcomes: [] as (boolean | string)[] };
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
              data: {
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
  await page.getByRole('combobox', { name: 'Darstellung', exact: true }).selectOption('dark');
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
