import { rm } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
await rm('.local/quality/browser-raw', { recursive: true, force: true });
const result = spawnSync(process.execPath, ['node_modules/@playwright/test/cli.js', 'test'], {
  stdio: 'inherit',
  env: { ...process.env, INSPECT_COVERAGE: '1' },
});
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);
await import('./frontend-coverage.mjs');
