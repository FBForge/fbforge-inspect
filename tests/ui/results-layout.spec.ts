import { test, expect, type Page } from './fixtures';
import { successfulDiagnosis } from '../fixtures/diagnosis-summary';
import { selectAppearance } from './theme';

const imageRoot =
  process.env.INSPECT_UPDATE_DOC_IMAGES === '1'
    ? 'docs/images'
    : '.local/quality/screenshots/ui-review';
const destination = '85a29591-22a6-49f3-9ac6-e31158aba022';
function realisticReport() {
  const report = successfulDiagnosis();
  report.appVersion = '0.2.0';
  report.environment = 'TEST';
  const titles: Record<string, string> = {
    DESTINATION_ID: 'Destination-ID',
    DESTINATION_STATUS: 'Destination-Status',
    DESTINATION_CONFIGURATION: 'Metadaten veröffentlicht',
    JWK_REQUIREMENTS: 'Öffentlicher Schlüssel',
    CERTIFICATE_BINDING: 'JWK und Zertifikat',
    CERTIFICATE_VALIDITY: 'Zertifikatslaufzeit',
    CERTIFICATE_SIGNATURE: 'Kettensignatur',
    CERTIFICATE_TRUST: 'V-PKI-Vertrauenskette',
    OCSP_STATUS: 'OCSP-Sperrauskunft',
    CRL_STATUS: 'CRL-Sperrauskunft',
  };
  for (const finding of report.findings) {
    finding.title = titles[finding.code!] || 'Einzelprüfung';
    finding.detail = 'Der geprüfte Einzelpunkt ist im fiktiven Diagnosebeispiel bestätigt.';
    finding.source = `https://test.fit-connect.fitko.de/submission-api/v3/destinations/${destination}`;
    finding.limitation = 'Fiktives Darstellungsbeispiel. Keine echte Prüfung eines Zustellpunkts.';
  }
  return report;
}
async function navigate(page: Page, name: string) {
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name, exact: true })
    .click();
}
async function screenshot(page: Page, name: string) {
  await page.screenshot({ path: `${imageRoot}/${name}.png` });
}

test('desktop navigation keeps complete words and fits a normal window', async ({ page }) => {
  await page.setViewportSize({ width: 1180, height: 800 });
  await page.goto('/');
  await selectAppearance(page, 'light');
  await navigate(page, 'Zertifikatsinventar');
  await screenshot(page, 'navigation');
  const sidebar = page.locator('.sidebar');
  expect(await sidebar.evaluate((el) => el.scrollHeight <= el.clientHeight + 1)).toBeTruthy();
  for (const name of ['Zertifikatsinventar', 'Fachdatenschemata']) {
    const label = sidebar.getByRole('button', { name, exact: true }).locator('span');
    expect(
      await label.evaluate((el) => {
        const range = document.createRange();
        range.selectNodeContents(el);
        return range.getClientRects().length;
      }),
    ).toBe(1);
  }
  for (const width of [920, 820]) {
    await page.setViewportSize({ width, height: 700 });
    const label = sidebar
      .getByRole('button', { name: 'Zertifikatsinventar', exact: true })
      .locator('span');
    expect(
      await label.evaluate((el) => {
        const range = document.createRange();
        range.selectNodeContents(el);
        return range.getClientRects().length;
      }),
    ).toBe(1);
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1),
    ).toBeTruthy();
  }
  await page.evaluate(() => {
    const sizes = [...document.querySelectorAll<HTMLElement>('.sidebar *')].map((el) => ({
      el,
      size: getComputedStyle(el).fontSize,
    }));
    for (const { el, size } of sizes) el.style.fontSize = `${parseFloat(size) * 2}px`;
  });
  expect(await sidebar.evaluate((el) => el.scrollWidth <= el.clientWidth + 1)).toBeTruthy();
});

test('batch presents full diagnosis through buttons without a wall of confirmed findings', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1360, height: 1000 });
  await page.addInitScript(
    (report) =>
      Object.assign(window, {
        isTauri: true,
        __TAURI_INTERNALS__: {
          invoke: async (command: string) =>
            command === 'diagnose' ? { ok: true, report } : false,
        },
      }),
    realisticReport(),
  );
  await page.goto('/');
  await selectAppearance(page, 'light');
  await navigate(page, 'Sammelprüfung');
  await page.getByLabel('Kennung oder öffentliche URL', { exact: true }).fill(destination);
  await page.getByRole('button', { name: 'Prüfung hinzufügen', exact: true }).click();
  await page.getByRole('button', { name: 'Sammelprüfung starten', exact: true }).click();
  await expect(page.getByText('Sammelprüfung abgeschlossen.', { exact: true })).toBeVisible();
  await page.getByText('Befunde anzeigen', { exact: true }).click();
  await page
    .getByRole('heading', { name: 'Prüfwarteschlange · TEST · v3' })
    .evaluate((el) => el.parentElement!.scrollIntoView({ block: 'start' }));
  await page
    .locator('.check-queue > li')
    .first()
    .evaluate((el) => el.scrollIntoView({ block: 'start' }));
  await screenshot(page, 'batch-diagnosis');
  const queue = page.locator('.check-queue');
  await expect(
    queue.getByRole('heading', { name: 'Geprüfte Konfiguration in Ordnung' }),
  ).toBeVisible();
  await expect(
    queue.getByRole('button', { name: 'Befunde ausblenden', exact: true }),
  ).toHaveAttribute('aria-expanded', 'true');
  await expect(queue.getByText('Destination-ID', { exact: true })).toBeHidden();
  await queue.getByRole('button', { name: /bestätigte Einzelprüfungen/ }).click();
  await expect(queue.getByText('Destination-ID', { exact: true })).toBeVisible();
  await queue.getByRole('button', { name: 'Beleg und Feldpfad', exact: true }).first().click();
  await expect(
    queue.getByText(realisticReport().findings[0]!.source, { exact: true }).first(),
  ).toBeVisible();
  await queue.getByRole('button', { name: 'Befunde ausblenden', exact: true }).click();
  await expect(
    queue.getByRole('heading', { name: 'Geprüfte Konfiguration in Ordnung' }),
  ).toBeHidden();
  await selectAppearance(page, 'dark');
  await queue.getByRole('button', { name: 'Befunde anzeigen', exact: true }).click();
  await page.screenshot({ path: '.local/quality/screenshots/ui-review/batch-diagnosis-dark.png' });
});

test('server diagnosis starts with a guided workflow and groups imported findings', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1360, height: 1000 });
  await page.goto('/');
  await selectAppearance(page, 'light');
  await navigate(page, 'Serverdiagnose');
  await page.getByRole('button', { name: 'Aufrufe und Rückgabecodes', exact: true }).click();
  await expect(page.getByText('PowerShell auf Windows:', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Aufrufe und Rückgabecodes', exact: true }).click();
  await screenshot(page, 'server-workflow');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Serverergebnis öffnen', exact: true }).click();
  await (
    await chooser
  ).setFiles({
    name: 'fiktives-serverergebnis.json',
    mimeType: 'application/json',
    buffer: Buffer.from(
      JSON.stringify({
        format: 'fbforge-inspect-result',
        formatVersion: 1,
        appVersion: '0.2.0',
        createdAt: '2026-11-06T12:00:00Z',
        exitCode: 0,
        runtime: { os: 'Windows Server', architecture: 'amd64', javaVersion: '25' },
        checks: [{ id: 'zustellpunkt', report: realisticReport() }],
      }),
    ),
  });
  await page
    .getByRole('heading', { name: 'Übertragenes Serverergebnis' })
    .evaluate((el) => el.parentElement!.scrollIntoView({ block: 'start' }));
  await screenshot(page, 'server-diagnosis');
  await expect(
    page.getByRole('heading', { name: 'Geprüfte Konfiguration in Ordnung' }),
  ).toBeVisible();
  await expect(page.getByText('Destination-ID', { exact: true })).toBeHidden();
  const details = page.getByRole('button', { name: /bestätigte Einzelprüfungen/ });
  await details.focus();
  await page.keyboard.press('Enter');
  await expect(details).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByText('Destination-ID', { exact: true })).toBeVisible();
  await page.keyboard.press('Space');
  await expect(details).toHaveAttribute('aria-expanded', 'false');
  await selectAppearance(page, 'dark');
  await page.screenshot({ path: '.local/quality/screenshots/ui-review/server-diagnosis-dark.png' });
});
