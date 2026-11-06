import { test, expect } from './fixtures';
import { selectAppearance } from './theme';
import { successfulDiagnosis } from '../fixtures/diagnosis-summary';
const id = '00000000-0000-4000-8000-000000000001';

test('capture current help screens from the application with explicitly synthetic fixtures', async ({
  page,
}) => {
  const diagnosis = successfulDiagnosis();
  diagnosis.appVersion = '0.2.0';
  await page.addInitScript((fixture) => {
    Object.assign(window, {
      isTauri: true,
      __TAURI_INTERNALS__: {
        invoke: async (command: string) =>
          command === 'diagnose'
            ? { ok: true, report: fixture }
            : {
                configured: true,
                supported: true,
                phase: 'idle',
                version: null,
                notes: '',
                downloaded: 0,
                total: null,
              },
      },
    });
  }, diagnosis);
  const imageRoot =
    process.env.INSPECT_UPDATE_DOC_IMAGES === '1'
      ? 'docs/images'
      : '.local/quality/screenshots/help';
  await page.goto('/');
  const navigate = async (name: string) =>
    page
      .getByRole('navigation', { name: 'Hauptnavigation' })
      .getByRole('button', { name, exact: true })
      .click();
  await selectAppearance(page, 'light');
  await page.screenshot({ path: `${imageRoot}/start.png` });
  for (const [name, file] of [
    ['Destination prüfen', 'destination'],
    ['Routing prüfen', 'routing'],
    ['Prozessnachricht prüfen', 'process'],
    ['Schlüssel prüfen', 'keys'],
    ['Netzwerk', 'network'],
  ] as const) {
    await navigate(name);
    await page.screenshot({ path: `${imageRoot}/${file}.png` });
  }
  await navigate('Prozessnachricht prüfen');
  await page.getByRole('button', { name: 'Limits prüfen', exact: true }).click();
  await page.screenshot({ path: `${imageRoot}/limits.png` });
  await navigate('Destination prüfen');
  await page.getByText('Zertifikate und Sperrstatus', { exact: false }).click();
  await page.screenshot({ path: `${imageRoot}/revocation.png` });
  await page
    .getByLabel('Destination-ID', { exact: true })
    .fill('85a29591-22a6-49f3-9ac6-e31158aba022');
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Geprüfte Konfiguration in Ordnung', exact: true }),
  ).toBeVisible();
  await page.locator('.result-overview').screenshot({ path: `${imageRoot}/diagnosis-summary.png` });
  await page.getByRole('button', { name: 'Bericht exportieren', exact: true }).click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.screenshot({ path: `${imageRoot}/reports.png` });
  await page.keyboard.press('Escape');
  await navigate('Sammelprüfung');
  await page.getByText('Mehrere Kennungen einfügen und Aufträge öffnen', { exact: true }).click();
  await page.getByRole('textbox', { name: 'Prüfeingaben' }).fill(`${id}\nnetwork`);
  await page.getByRole('button', { name: 'Prüfungen vorbereiten' }).click();
  await page.getByText('Mehrere Kennungen einfügen und Aufträge öffnen', { exact: true }).click();
  await page.screenshot({ path: `${imageRoot}/batch.png` });
  await navigate('Werkzeuge');
  await page.getByRole('combobox', { name: 'Aufgabe', exact: true }).selectOption('reverse');
  await page.getByRole('textbox', { name: 'Destination-ID', exact: true }).fill(id);
  await page.screenshot({ path: `${imageRoot}/tools.png` });
  const cert = {
    chainIndex: 0,
    sha256: 'a'.repeat(64),
    kid: 'beispielschluessel',
    subject: 'CN=Fiktive Beispielverwaltung',
    issuer: 'CN=Fiktive Beispiel-CA',
    notBefore: '2026-01-01T00:00:00Z',
    notAfter: '2027-01-01T00:00:00Z',
  };
  const report = {
    protocolVersion: 1,
    appVersion: '0.2.0',
    createdAt: '2026-11-06T12:00:00Z',
    operation: 'destination',
    environment: 'TEST',
    apiVersion: 'v3',
    requests: [],
    findings: [
      {
        status: 'unknown',
        title: 'Fiktives Serverergebnis',
        detail: 'Diese Darstellung erläutert die Auswertung eines übertragenen Prüfbelegs.',
        path: '$',
        source: '',
        nextStep: '',
        limitation: 'Fiktives Dokumentationsbeispiel.',
      },
    ],
    data: {
      destination: { destinationId: id, encryptionKid: 'beispielschluessel' },
      certificates: [cert],
    },
  };
  const result = {
    format: 'fbforge-inspect-result',
    formatVersion: 1,
    appVersion: '0.2.0',
    createdAt: '2026-11-06T12:00:00Z',
    runtime: { os: 'Linux', architecture: 'aarch64', javaVersion: '25' },
    checks: [{ id: 'beispiel-server', report }],
    exitCode: 3,
  };
  await navigate('Serverdiagnose');
  await page.getByLabel('Serverergebnis öffnen', { exact: true }).setInputFiles({
    name: 'fiktives-serverergebnis.json',
    mimeType: 'application/json',
    buffer: Buffer.from(JSON.stringify(result)),
  });
  await expect(page.getByText('Linux · aarch64 · Java 25', { exact: true })).toBeVisible();
  await page.screenshot({ path: `${imageRoot}/server.png` });
  await navigate('Zertifikatsinventar');
  await expect(page.getByRole('heading', { name: 'CN=Fiktive Beispielverwaltung' })).toBeVisible();
  await page.screenshot({ path: `${imageRoot}/inventory.png` });
  await navigate('Info');
  await page.getByRole('heading', { name: 'Inspect aktualisieren' }).scrollIntoViewIfNeeded();
  await page.screenshot({ path: `${imageRoot}/updates.png` });
  for (const appearance of ['light', 'dark']) {
    await selectAppearance(page, appearance);
    await navigate('Sammelprüfung');
    await page.screenshot({ path: `.local/quality/screenshots/0.2.0-batch-${appearance}.png` });
    await navigate('Serverdiagnose');
    await page.screenshot({ path: `.local/quality/screenshots/0.2.0-server-${appearance}.png` });
    await page.setViewportSize({ width: 860, height: 720 });
    await navigate('Sammelprüfung');
    await expect
      .poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth))
      .toBe(true);
    await page.screenshot({
      path: `.local/quality/screenshots/0.2.0-batch-compact-${appearance}.png`,
    });
    await page.setViewportSize({ width: 1180, height: 800 });
    await navigate('Zertifikatsinventar');
    await page.screenshot({ path: `.local/quality/screenshots/0.2.0-inventory-${appearance}.png` });
  }
});
