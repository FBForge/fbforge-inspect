import { test, expect, type Page } from '@playwright/test';

const pages = [
  'Start',
  'Destination prüfen',
  'Routing prüfen',
  'Prozessnachricht prüfen',
  'Schlüssel prüfen',
  'Fachdatenschemata',
  'Hilfe',
  'Info',
];
async function selectPage(page: Page, name: string) {
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name, exact: true })
    .click();
}
async function noOverflow(page: Page) {
  expect(
    await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1),
  ).toBeTruthy();
  const outside = await page.evaluate(() => {
    const header = document.querySelector('.topbar')!.getBoundingClientRect();
    return [...document.querySelectorAll('.toolbar label')].some((element) => {
      const box = element.getBoundingClientRect();
      return (
        box.top < header.top - 1 || box.bottom > header.bottom + 1 || box.right > innerWidth + 1
      );
    });
  });
  expect(outside).toBeFalsy();
}

test('small brand text and input placeholders retain readable contrast in both appearances', async ({
  page,
}) => {
  await page.goto('/');
  await selectPage(page, 'Fachdatenschemata');
  for (const theme of ['light', 'dark']) {
    await page.getByRole('combobox', { name: 'Darstellung', exact: true }).selectOption(theme);
    const ratios = await page.evaluate(() => {
      const lum = (color: string) =>
        color
          .match(/[\d.]+/g)!
          .slice(0, 3)
          .map(Number)
          .map((c) => {
            const v = c / 255;
            return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
          })
          .reduce((s, c, i) => s + c * [0.2126, 0.7152, 0.0722][i]!, 0);
      const ratio = (a: string, b: string) =>
        (Math.max(lum(a), lum(b)) + 0.05) / (Math.min(lum(a), lum(b)) + 0.05);
      const brand = getComputedStyle(document.querySelector('.eyebrow')!);
      const input = document.querySelector('input[type="url"]')!;
      const placeholder = getComputedStyle(input, '::placeholder');
      return {
        brand: ratio(brand.color, getComputedStyle(document.body).backgroundColor),
        placeholder: ratio(placeholder.color, getComputedStyle(input).backgroundColor),
        opacity: placeholder.opacity,
      };
    });
    expect(ratios.brand, `${theme} small brand text`).toBeGreaterThanOrEqual(4.5);
    expect(ratios.placeholder, `${theme} placeholder`).toBeGreaterThanOrEqual(4.5);
    expect(ratios.opacity).toBe('1');
  }
});

test('every page reflows at the CSS width corresponding to 200 percent zoom in a minimum window', async ({
  page,
}) => {
  // This is CSS reflow coverage, not a native Windows DPI acceptance claim.
  await page.setViewportSize({ width: 350, height: 540 });
  await page.goto('/');
  for (const theme of ['light', 'dark']) {
    await page.getByRole('combobox', { name: 'Darstellung', exact: true }).selectOption(theme);
    for (const name of pages) {
      await selectPage(page, name);
      await noOverflow(page);
    }
  }
});

test('200 percent text enlargement keeps every page and toolbar usable', async ({ page }) => {
  await page.setViewportSize({ width: 700, height: 540 });
  await page.goto('/');
  for (const theme of ['light', 'dark']) {
    await page.getByRole('combobox', { name: 'Darstellung', exact: true }).selectOption(theme);
    for (const name of pages) {
      await selectPage(page, name);
      await page.evaluate(() => {
        const sizes = [...document.querySelectorAll<HTMLElement>('body *')].map((e) => ({
          e,
          size: getComputedStyle(e).fontSize,
        }));
        for (const { e, size } of sizes) e.style.fontSize = `${parseFloat(size) * 2}px`;
      });
      await noOverflow(page);
      await page.evaluate(() =>
        document
          .querySelectorAll<HTMLElement>('body *')
          .forEach((e) => e.style.removeProperty('font-size')),
      );
    }
  }
});

test('keyboard navigation reaches the last sidebar action in a short window', async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 540 });
  await page.goto('/');
  let reached = false;
  for (let i = 0; i < 16; i++) {
    await page.keyboard.press('Tab');
    const state = await page.evaluate(() => {
      const e = document.activeElement!;
      const box = e.getBoundingClientRect();
      return {
        name: e.getAttribute('aria-label'),
        visible: box.top >= 0 && box.bottom <= innerHeight,
      };
    });
    if (state.name === 'Info') {
      expect(state.visible).toBeTruthy();
      reached = true;
      break;
    }
  }
  expect(reached).toBeTruthy();
});

test('long results and export remain readable by keyboard at narrow widths', async ({ page }) => {
  await page.addInitScript(() =>
    Object.assign(window, {
      isTauri: true,
      __TAURI_INTERNALS__: {
        invoke: async (command: string) => {
          if (command === 'save_report') return false;
          return {
            ok: true,
            report: {
              protocolVersion: 1,
              appVersion: '0.1.0',
              createdAt: '2026-10-30T12:00:00Z',
              operation: 'schema',
              environment: 'TEST',
              apiVersion: 'v3',
              requests: [],
              findings: [
                {
                  status: 'unknown',
                  title: 'PVOG-Leistungsbeschreibungszuordnung',
                  detail: 'Synthetischer Umbruchtest',
                  path: '$',
                  source: '',
                  nextStep: '',
                  limitation: '',
                },
              ],
              data: {
                dialect: 'XSD',
                structure: Array.from({ length: 100 }, (_, i) => ({
                  path: `$.property${i}`,
                  type: 'string',
                })),
              },
            },
          };
        },
      },
    }),
  );
  await page.setViewportSize({ width: 350, height: 540 });
  await page.goto('/');
  await selectPage(page, 'Fachdatenschemata');
  await page.getByRole('textbox', { name: 'JSON Schema / XSD', exact: true }).fill('{}');
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  const structure = page.getByRole('region', { name: 'Schemastruktur', exact: true });
  await structure.focus();
  await page.keyboard.press('ArrowDown');
  await expect.poll(() => structure.evaluate((e) => e.scrollTop)).toBeGreaterThan(0);
  await noOverflow(page);
  await page.getByRole('button', { name: 'Bericht exportieren', exact: true }).click();
  const preview = page.locator('.export-preview');
  await preview.focus();
  await page.keyboard.press('ArrowDown');
  await expect.poll(() => preview.evaluate((e) => e.scrollTop)).toBeGreaterThan(0);
  const dialog = page.getByRole('dialog');
  for (let i = 0; i < 12; i++) {
    await page.keyboard.press('Tab');
    expect(await dialog.evaluate((e) => e.contains(document.activeElement))).toBeTruthy();
  }
  for (let i = 0; i < 8; i++) {
    await page.keyboard.press('Shift+Tab');
    expect(await dialog.evaluate((e) => e.contains(document.activeElement))).toBeTruthy();
  }
  await page.keyboard.press('Escape');
  await expect(
    page.getByRole('button', { name: 'Bericht exportieren', exact: true }),
  ).toBeFocused();
});

test('forced colors keep environment labels visible and reduced motion stops the busy spinner', async ({
  page,
}) => {
  await page.emulateMedia({ forcedColors: 'active', reducedMotion: 'reduce' });
  await page.addInitScript(() =>
    Object.assign(window, {
      isTauri: true,
      __TAURI_INTERNALS__: { invoke: () => new Promise(() => {}) },
    }),
  );
  await page.goto('/');
  await expect(page.locator('.environment-tag')).toContainText('TEST');
  await selectPage(page, 'Fachdatenschemata');
  await page.getByRole('textbox', { name: 'JSON Schema / XSD', exact: true }).fill('{}');
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await expect(page.locator('.busy-panel')).toBeVisible();
  await expect(page.locator('.spinner')).toHaveCSS('animation-name', 'none');
});
