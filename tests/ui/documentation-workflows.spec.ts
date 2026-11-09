import { createHash } from 'node:crypto';
import { test, expect, type Page } from './fixtures';
import { selectAppearance } from './theme';

test.use({ viewport: { width: 1360, height: 1000 } });
const imageRoot =
  process.env.INSPECT_UPDATE_DOC_IMAGES === '1' ? 'docs/images' : '.local/quality/screenshots/help';
const destination = '00000000-0000-4000-8000-00000000000';
const finding = (status: string, title: string, detail: string, nextStep = '') => ({
  status,
  title,
  detail,
  nextStep,
  path: '$',
  source: '',
  limitation: 'Fiktives Dokumentationsbeispiel.',
});
const report = (operation: string, findings: object[], data: object = {}) => ({
  protocolVersion: 1,
  appVersion: '0.2.0',
  createdAt: '2026-11-06T12:00:00Z',
  operation,
  environment: 'TEST',
  apiVersion: 'v3',
  findings,
  data,
  requests: [],
});
async function navigate(page: Page, name: string) {
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name, exact: true })
    .click();
}
async function bridge(page: Page, reports: Record<string, object>) {
  await page.addInitScript((fixtures) => {
    Object.assign(window, {
      isTauri: true,
      __TAURI_INTERNALS__: {
        invoke: async (command: string, args: any) =>
          command === 'diagnose'
            ? { ok: true, report: fixtures[args.request.input.destinationId || 'network'] }
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
  }, reports);
}
async function capture(page: Page, name: string) {
  await page.screenshot({ path: `${imageRoot}/${name}.png` });
  await selectAppearance(page, 'dark');
  await page.screenshot({ path: `.local/quality/screenshots/help/${name}-dark.png` });
}

test('illustrated batch guide exposes the failed entry and its next step', async ({ page }) => {
  await bridge(page, {
    [destination + '1']: report('destination', [
      finding(
        'confirmed',
        'Geprüfte Einzelpunkte bestätigt',
        'Fiktives Beispiel eines bestätigten Zustellpunkts.',
      ),
    ]),
    [destination + '2']: report('destination', [
      finding(
        'error',
        'Zertifikat abgelaufen',
        'Das Teilnehmerzertifikat ist im fiktiven Beispiel zeitlich nicht mehr gültig.',
        'Ersatzschlüssel und zugehörige Zertifikatskette beim Betreiber prüfen.',
      ),
    ]),
    [destination + '3']: report('destination', [
      finding(
        'unknown',
        'Sperrstatus nicht nachgewiesen',
        'Die Sperrauskunft ist im fiktiven Beispiel nicht erreichbar.',
        'Verbindungsweg zur Sperrauskunft prüfen und diesen Eintrag erneut ausführen.',
      ),
    ]),
  });
  await page.goto('/');
  await selectAppearance(page, 'light');
  await navigate(page, 'Sammelprüfung');
  for (const suffix of ['1', '2', '3']) {
    await page
      .getByLabel('Kennung oder öffentliche URL', { exact: true })
      .fill(destination + suffix);
    await page.getByRole('button', { name: 'Prüfung hinzufügen', exact: true }).click();
  }
  await page.getByRole('button', { name: 'Sammelprüfung starten', exact: true }).click();
  await expect(page.getByText('Sammelprüfung abgeschlossen.', { exact: true })).toBeVisible();
  await expect(page.getByText('1 mit Handlungsbedarf', { exact: false })).toBeVisible();
  await page
    .getByRole('group', { name: 'Prüfwarteschlange filtern' })
    .getByRole('button', { name: /^Handlungsbedarf/ })
    .click();
  await expect(page.locator('.check-queue > li')).toHaveCount(1);
  await expect(page.locator('.check-queue')).toContainText('Zertifikat abgelaufen');
  await expect(page.locator('.check-queue')).toContainText(
    'Ersatzschlüssel und zugehörige Zertifikatskette',
  );
  await page
    .getByRole('heading', { name: 'Prüfwarteschlange · TEST · v3' })
    .evaluate((el) => el.scrollIntoView({ block: 'start' }));
  await capture(page, 'batch-results');
});

test('illustrated server guide shows a matching job fingerprint and its limits', async ({
  page,
}) => {
  await page.goto('/');
  await selectAppearance(page, 'light');
  await navigate(page, 'Serverdiagnose');
  const job = JSON.stringify({
    format: 'fbforge-inspect-job',
    formatVersion: 1,
    environment: 'TEST',
    apiVersion: 'v3',
    checks: [{ id: 'verbindung', operation: 'network', input: {} }],
  });
  await page.getByLabel('Auftragsdatei zur Zuordnung öffnen', { exact: true }).setInputFiles({
    name: 'auftrag.json',
    mimeType: 'application/json',
    buffer: Buffer.from(job),
  });
  await page.getByLabel('Serverergebnis öffnen', { exact: true }).setInputFiles({
    name: 'fiktives-serverergebnis.json',
    mimeType: 'application/json',
    buffer: Buffer.from(
      JSON.stringify({
        format: 'fbforge-inspect-result',
        formatVersion: 1,
        appVersion: '0.2.0',
        createdAt: '2026-11-06T12:00:00Z',
        runtime: { os: 'Linux', architecture: 'amd64', javaVersion: '25' },
        jobSha256: createHash('sha256').update(job).digest('hex'),
        exitCode: 0,
        checks: [
          {
            id: 'verbindung',
            report: report('network', [
              finding(
                'confirmed',
                'Fiktiver Verbindungsnachweis',
                'Dieses lokale Beispiel zeigt die Darstellung eines übertragenen Prüfbelegs.',
              ),
            ]),
          },
        ],
      }),
    ),
  });
  await expect(
    page.getByText('Ergebnis passt zu den Bytes der ausgewählten Auftragsdatei.', { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText(
      'Der Fingerabdruck ordnet Dateien einander zu. Er bestätigt keine Serveridentität.',
      { exact: true },
    ),
  ).toBeVisible();
  await page
    .getByRole('heading', { name: 'Übertragenes Serverergebnis' })
    .evaluate((el) => el.parentElement!.scrollIntoView({ block: 'start' }));
  await capture(page, 'server-association');
});

test('illustrated network guide locates a proxy failure before target TLS', async ({ page }) => {
  const url = 'https://example.org/';
  const failure = finding(
    'unknown',
    'Proxy-Anmeldung erforderlich',
    'Der Proxy fordert im fiktiven Beispiel eine Anmeldung an. Die Verbindung zum Zielserver wurde nicht nachgewiesen.',
    'Proxy-Adresse und Anmeldung unter Netzwerk prüfen. Anschließend das betroffene Ziel erneut prüfen.',
  );
  failure.source = url;
  await bridge(page, {
    network: report('network', [failure], {
      networkContext: { httpsTrust: 'Fiktives Beispiel mit manuellem Proxy.' },
      networkTraces: [
        {
          url,
          route: 'manualProxy',
          phases: [
            { stage: 'dns', outcome: 'confirmed', durationMs: 2 },
            { stage: 'proxyDns', outcome: 'confirmed', durationMs: 1 },
            { stage: 'proxyTcp', outcome: 'confirmed', durationMs: 3 },
            { stage: 'proxyConnect', outcome: 'PROXY_AUTH_REQUIRED', durationMs: 2 },
          ],
        },
      ],
    }),
  });
  await page.goto('/');
  await selectAppearance(page, 'light');
  await navigate(page, 'Netzwerk');
  await page.getByRole('combobox', { name: 'Verbindung', exact: true }).selectOption('manual');
  await page.getByLabel('Proxy-Adresse', { exact: true }).fill('https://proxy.example.org:8080');
  await page.getByRole('button', { name: 'Verbindungen prüfen', exact: true }).click();
  const guidance = page.getByRole('region', { name: 'Fehlgeschlagene Verbindungsphasen' });
  await expect(guidance.getByText('Proxy-Tunnel und Anmeldung', { exact: true })).toBeVisible();
  await expect(guidance).toContainText(
    'Nachfolgende Schritte dieser Verbindung sind nicht nachgewiesen.',
  );
  await guidance.evaluate((el) => el.scrollIntoView({ block: 'start' }));
  await capture(page, 'network-failure');
});
