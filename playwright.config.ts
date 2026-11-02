import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: 'tests/ui',
  fullyParallel: true,
  retries: 0,
  use: {
    baseURL: 'http://127.0.0.1:1421',
    viewport: { width: 1180, height: 800 },
    headless: true,
    launchOptions: { executablePath: process.env.INSPECT_CHROMIUM_EXECUTABLE },
  },
  webServer: { command: 'npm run dev', url: 'http://127.0.0.1:1421', reuseExistingServer: true },
});
