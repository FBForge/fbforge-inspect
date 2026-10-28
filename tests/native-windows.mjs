// Manual invocation only. Uses a temporary WebView2 profile and loopback debugging for this process.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdir, mkdtemp, writeFile, readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { join, dirname, resolve } from 'node:path';
import net from 'node:net';
if (process.platform !== 'win32') throw new Error('This probe requires Windows.');
if (!process.argv[2] || !process.argv[3])
  throw new Error('Usage: node tests/native-windows.mjs <exe> <evidence-dir>');
const executable = resolve(process.argv[2]),
  output = resolve(process.argv[3]);
await mkdir(output, { recursive: true });
const profile = await mkdtemp(join(tmpdir(), 'inspect-webview-test-'));
const port = await new Promise((resolve, reject) => {
  const server = net.createServer();
  server.once('error', reject);
  server.listen(0, '127.0.0.1', () => {
    const address = server.address();
    server.close(() => resolve(address.port));
  });
});
const child = spawn(executable, [], {
  cwd: dirname(executable),
  env: {
    ...process.env,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
    WEBVIEW2_USER_DATA_FOLDER: profile,
  },
  stdio: 'ignore',
});
const results = [];
let browser;
try {
  const deadline = Date.now() + 30000;
  while (Date.now() < deadline) {
    try {
      browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1500 });
      break;
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
  }
  if (!browser) throw new Error('Native WebView2 did not expose the test endpoint.');
  const context = browser.contexts()[0];
  let page;
  for (let attempt = 0; attempt < 100; attempt++) {
    page = context
      .pages()
      .find(
        (p) =>
          p.url().startsWith('http://tauri.localhost') || p.url().startsWith('tauri://localhost'),
      );
    if (page) break;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  if (!page) throw new Error('Own native app page not found.');
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await expect(page.getByRole('heading', { name: 'Klarheit für Ihre Integration.' })).toBeVisible();
  results.push('Native WebView2 frontend loaded');
  await page.screenshot({ path: join(output, 'windows-start.png') });
  await page
    .getByRole('navigation')
    .getByRole('button', { name: 'Fachschemata', exact: true })
    .click();
  await page
    .getByRole('textbox', { name: 'JSON Schema / XSD' })
    .fill('{"type":"object","properties":{"d":{"type":"string"}}}');
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Schema eingelesen', exact: true })).toBeVisible({
    timeout: 15000,
  });
  results.push('Native UI → Rust → bundled Java → result round trip passed');
  await page.getByRole('button', { name: 'Bericht exportieren' }).click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await expect(page.getByRole('dialog').locator('pre')).not.toContainText('rawResponses');
  await page.keyboard.press('Escape');
  results.push('Native export preview excludes raw responses by default');
  await page.getByRole('button', { name: 'Dunkles Design aktivieren' }).click();
  await page.screenshot({ path: join(output, 'windows-schema-dark.png') });
  await page
    .getByRole('navigation')
    .getByRole('button', { name: 'Schlüssel prüfen', exact: true })
    .click();
  await page
    .getByRole('textbox', { name: 'Öffentliches JWK / JWKS' })
    .fill('{"kty":"RSA","d":"synthetic-private-marker"}');
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Private oder symmetrische Schlüssel');
  results.push('Native private-key rejection passed');
  if (errors.length)
    throw new Error('Native renderer reported runtime errors: ' + errors.join('\n'));
  const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
  await writeFile(
    join(output, 'native-windows-results.json'),
    JSON.stringify(
      {
        ok: true,
        at: new Date().toISOString(),
        applicationSha256: sha256(await readFile(executable)),
        engineSha256: sha256(
          await readFile(join(dirname(executable), 'engine/inspect-engine.jar')),
        ),
        results,
        rendererErrors: errors,
      },
      null,
      2,
    ),
  );
  console.log(JSON.stringify({ ok: true, results }));
} finally {
  if (browser) await browser.close();
  child.kill();
}
