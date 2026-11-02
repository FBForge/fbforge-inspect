import { test as base } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';
export { expect, type Page } from '@playwright/test';

/** Coverage is opt-in; the same assertions run with and without instrumentation. */
export const test = base.extend({
  page: async ({ page }, use, info) => {
    if (process.env.INSPECT_COVERAGE !== '1') {
      await use(page);
      return;
    }
    await page.coverage.startJSCoverage({ resetOnNavigation: false });
    await use(page);
    const entries = await page.coverage.stopJSCoverage();
    const directory = resolve('.local/quality/browser-raw');
    await mkdir(directory, { recursive: true });
    const key = createHash('sha256').update(info.testId).digest('hex').slice(0, 16);
    await writeFile(resolve(directory, key + '.json'), JSON.stringify(entries));
  },
});
