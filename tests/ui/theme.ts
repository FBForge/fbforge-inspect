import type { Page } from './fixtures';
export async function selectAppearance(page: Page, value: string) {
  await page.getByRole('button', { name: 'Darstellung wählen', exact: true }).click();
  await page
    .getByRole('group', { name: 'Darstellung', exact: true })
    .getByRole('button', {
      name: { system: 'System', light: 'Hell', dark: 'Dunkel' }[value],
      exact: true,
    })
    .click();
}
