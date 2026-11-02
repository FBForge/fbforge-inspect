import { test, expect } from './fixtures';
import { summaryScenarios } from '../fixtures/diagnosis-summary';

for (const { name, report, title, text } of summaryScenarios()) {
  test(`Ergebniszusammenfassung: ${name}`, async ({ page }) => {
    await page.addInitScript(
      (report) =>
        Object.assign(window, {
          isTauri: true,
          __TAURI_INTERNALS__: {
            invoke: async (command: string) =>
              command === 'diagnose' ? { ok: true, report } : undefined,
          },
        }),
      report,
    );
    await page.goto('/');
    await page
      .getByRole('navigation', { name: 'Hauptnavigation' })
      .getByRole('button', { name: 'Destination prüfen', exact: true })
      .click();
    await page
      .getByLabel('Destination-ID', { exact: true })
      .fill('85a29591-22a6-49f3-9ac6-e31158aba022');
    await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
    const summary = page.getByLabel('Zusammenfassung der Prüfbereiche');
    await expect(page.getByRole('heading', { name: title, exact: true })).toBeVisible();
    await expect(page.locator('.result-overview')).toContainText(text);
    await expect(summary).toBeVisible();
    await expect(summary).toContainText('Schlüssel');
    await page.getByRole('button', { name: /\d+ Bestätigt/, exact: true }).click();
    await expect(page.getByRole('heading', { name: title, exact: true })).toBeVisible();
    await expect(summary).toBeVisible();
    if (name === 'vollständig bestätigt') {
      await page.screenshot({ path: 'test-results/diagnosis-summary-desktop.png', fullPage: true });
      await page.setViewportSize({ width: 390, height: 844 });
      await expect(summary).toBeVisible();
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
        true,
      );
      await page.screenshot({ path: 'test-results/diagnosis-summary-mobile.png', fullPage: true });
    }
  });
}
