import { test, expect } from '@playwright/test';
test('system changes apply live, manual choice survives system changes and reload', async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: 'light' });
  await page.goto('/');
  const picker = page.getByRole('combobox', { name: 'Darstellung', exact: true });
  await expect(picker).toHaveValue('system');
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await picker.selectOption('light');
  await page.emulateMedia({ colorScheme: 'light' });
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.reload();
  await expect(picker).toHaveValue('light');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await picker.selectOption('system');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});
test('unavailable preference storage does not block theme selection or navigation', async ({
  page,
}) => {
  await page.addInitScript(() => {
    Storage.prototype.getItem = () => {
      throw new Error('Storage unavailable');
    };
    Storage.prototype.setItem = () => {
      throw new Error('Storage unavailable');
    };
  });
  await page.goto('/');
  await page.getByRole('combobox', { name: 'Darstellung', exact: true }).selectOption('dark');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Destination prüfen', exact: true })
    .click();
  await expect(
    page.getByRole('heading', { name: 'Destination prüfen', exact: true }),
  ).toBeFocused();
});
test('toolbar labels remain inside the header at minimum and standard window sizes', async ({
  page,
}) => {
  for (const width of [700, 1180]) {
    await page.setViewportSize({ width, height: 800 });
    await page.goto('/');
    const header = await page.locator('.topbar').boundingBox();
    for (const name of ['Umgebung', 'API', 'Darstellung']) {
      const box = await page
        .getByRole('combobox', { name, exact: true })
        .locator('..')
        .boundingBox();
      expect(box!.y).toBeGreaterThanOrEqual(header!.y);
      expect(box!.y + box!.height).toBeLessThanOrEqual(header!.y + header!.height);
    }
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth),
    ).toBeTruthy();
  }
});

test('enabled form fields have a visible boundary in both themes', async ({ page }) => {
  await page.goto('/');
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Destination prüfen', exact: true })
    .click();
  for (const theme of ['light', 'dark']) {
    await page.getByRole('combobox', { name: 'Darstellung', exact: true }).selectOption(theme);
    const ratio = await page
      .getByRole('textbox', { name: 'Destination-ID', exact: true })
      .evaluate((element) => {
        const style = getComputedStyle(element);
        const luminance = (color: string) => {
          const channels = color
            .match(/[\d.]+/g)!
            .slice(0, 3)
            .map(Number)
            .map((value) => {
              const s = value / 255;
              return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
            });
          return channels[0]! * 0.2126 + channels[1]! * 0.7152 + channels[2]! * 0.0722;
        };
        const a = luminance(style.borderTopColor),
          b = luminance(style.backgroundColor);
        return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
      });
    expect(ratio, `${theme} field boundary`).toBeGreaterThanOrEqual(3);
  }
});

test('offline help includes schema steps and a usable synthetic example', async ({ page }) => {
  await page.goto('/');
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Hilfe', exact: true })
    .click();
  await page
    .getByRole('navigation', { name: 'Hilfethemen' })
    .getByRole('link', { name: 'Fachdatenschemata lesen', exact: true })
    .click();
  const topic = page.locator('#schemas');
  await expect(topic.locator('ol li')).toHaveCount(3);
  const example = JSON.parse(await topic.locator('pre').innerText());
  expect(example.type).toBe('object');
  expect(example.properties.vorgangsnummer.type).toBe('string');
});
