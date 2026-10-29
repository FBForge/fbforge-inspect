// Manual invocation only. Uses a temporary WebView2 profile and loopback debugging for this process.
import { chromium, expect } from '@playwright/test';
import { spawn, spawnSync, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
import { mkdir, mkdtemp, writeFile, readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { join, dirname, resolve } from 'node:path';
import net from 'node:net';
import { reportHtml } from '../frontend/src/reports.ts';
if (process.platform !== 'win32') throw new Error('This probe requires Windows.');
if (!process.argv[2] || !process.argv[3])
  throw new Error('Usage: node tests/native-windows.mjs <exe> <evidence-dir>');
const executable = resolve(process.argv[2]),
  output = resolve(process.argv[3]);
await mkdir(output, { recursive: true });
const profile = await mkdtemp(join(process.env.RUNNER_TEMP || tmpdir(), 'inspect-webview-test-'));
const port = await new Promise((resolve, reject) => {
  const server = net.createServer();
  server.once('error', reject);
  server.listen(0, '127.0.0.1', () => {
    const address = server.address();
    server.close(() => resolve(address.port));
  });
});
const hosted = process.env.RUNNER_ENVIRONMENT === 'github-hosted';
const policyScript = fileURLToPath(
  new URL('../scripts/ci/webview-test-policy.ps1', import.meta.url),
);
function testPolicy(mode) {
  const result = spawnSync(
    join(process.env.SystemRoot, 'System32/WindowsPowerShell/v1.0/powershell.exe'),
    [
      '-NoProfile',
      '-NonInteractive',
      '-File',
      policyScript,
      '-Mode',
      mode,
      '-Directory',
      profile,
      '-Port',
      String(port),
    ],
    { encoding: 'utf8', timeout: 15000 },
  );
  if (result.stdout) process.stdout.write(result.stdout);
  if (result.error || result.status !== 0)
    throw new Error(
      `Temporary WebView2 test policy ${mode} failed: ${result.stderr || result.error?.message || result.status}`,
    );
}
if (hosted) {
  try {
    testPolicy('enable');
  } catch (error) {
    testPolicy('disable');
    throw error;
  }
}
const child = spawn(executable, [], {
  cwd: dirname(executable),
  env: {
    ...process.env,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
    WEBVIEW2_USER_DATA_FOLDER: profile,
  },
  stdio: ['ignore', 'ignore', 'pipe'],
});
let startupError = '';
child.on('error', (error) => {
  startupError = error.message;
});
child.stderr.on('data', (bytes) => {
  startupError = (startupError + bytes.toString('utf8')).slice(-4096);
});
const results = [];
let browser;
try {
  const deadline = Date.now() + 60000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null || child.signalCode !== null)
      throw new Error(
        `Native app exited before WebView2 was ready: ${startupError || child.exitCode || child.signalCode}`,
      );
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
          p.url().startsWith('http://tauri.localhost/') ||
          p.url().startsWith('https://tauri.localhost/') ||
          p.url().startsWith('tauri://localhost/'),
      );
    if (page) break;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  if (!page)
    throw new Error(
      'Own native app page not found: ' +
        context
          .pages()
          .map((p) => p.url())
          .join(', '),
    );
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await expect(page.getByRole('heading', { name: 'Klarheit für Ihre Integration.' })).toBeVisible({
    timeout: 20000,
  });
  results.push('Native WebView2 frontend loaded');
  await page.screenshot({ path: join(output, 'windows-start.png') });
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Fachdatenschemata', exact: true })
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
  const reportDirectory = join(profile, 'saved-reports');
  await mkdir(reportDirectory);
  const controlSaveDialog = async (mode, fileName) => {
    const args = [
      '-NoProfile',
      '-NonInteractive',
      '-File',
      fileURLToPath(new URL('./native-save-dialog.ps1', import.meta.url)),
      '-ApplicationProcessId',
      String(child.pid),
      '-Mode',
      mode,
      '-TestDirectory',
      reportDirectory,
    ];
    if (fileName) args.push('-FileName', fileName);
    const result = await promisify(execFile)(
      join(process.env.SystemRoot, 'System32/WindowsPowerShell/v1.0/powershell.exe'),
      args,
      {
        encoding: 'utf8',
        timeout: 35000,
        windowsHide: true,
        env: { ...process.env, RUNNER_TEMP: process.env.RUNNER_TEMP || tmpdir() },
      },
    );
    process.stdout.write(result.stdout);
  };
  await page.getByRole('button', { name: 'Bericht exportieren' }).click();
  const exportDialog = page.getByRole('dialog');
  await exportDialog.getByRole('button', { name: 'Speicherort wählen' }).click();
  await controlSaveDialog('cancel');
  await expect(exportDialog).toBeVisible();
  await expect(exportDialog.getByRole('button', { name: 'Speicherort wählen' })).toBeEnabled();
  results.push('Cancelling the real Windows Save dialog preserves the export preview');
  await page.screenshot({ path: join(output, 'windows-export-preview.png') });
  for (const format of ['json', 'html']) {
    for (const raw of [false, true]) {
      if (!(await exportDialog.isVisible()))
        await page.getByRole('button', { name: 'Bericht exportieren' }).click();
      await exportDialog
        .getByRole('combobox', { name: 'Format', exact: true })
        .selectOption(format);
      await exportDialog
        .getByRole('checkbox', { name: 'Rohantworten einschließen' })
        .setChecked(raw);
      const preview = JSON.parse(await exportDialog.locator('pre').innerText());
      if (raw) expect(preview).toHaveProperty('rawResponses');
      else expect(preview.rawResponses).toBeUndefined();
      const fileName = `native-report-${raw ? 'raw' : 'default'}.${format}`;
      await exportDialog.getByRole('button', { name: 'Speicherort wählen' }).click();
      await controlSaveDialog('save', fileName);
      await expect(exportDialog).toHaveCount(0);
      const saved = await readFile(join(reportDirectory, fileName), 'utf8');
      expect(saved).toBe(
        format === 'json' ? JSON.stringify(preview, null, 2) : reportHtml(preview),
      );
      await expect(page.getByRole('button', { name: 'Bericht exportieren' })).toBeFocused();
      results.push(
        `Native ${format.toUpperCase()} Save dialog and file contents passed (${raw ? 'raw enabled' : 'raw excluded'})`,
      );
    }
  }
  await page.getByRole('combobox', { name: 'Darstellung', exact: true }).selectOption('dark');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.screenshot({ path: join(output, 'windows-schema-dark.png') });
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Schlüssel prüfen', exact: true })
    .click();
  await page
    .getByRole('textbox', { name: 'Öffentliches JWK / JWKS' })
    .fill('{"kty":"RSA","d":"synthetic-private-marker"}');
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Private oder symmetrische Schlüssel');
  results.push('Native private-key rejection passed');
  if (process.env.INSPECT_LIVE_READONLY === '1') {
    await page.getByRole('combobox', { name: 'Umgebung', exact: true }).selectOption('TEST');
    await page
      .getByRole('navigation', { name: 'Hauptnavigation' })
      .getByRole('button', { name: 'Routing prüfen', exact: true })
      .click();
    await page.getByText('Gebiet über Ortsnamen finden', { exact: true }).click();
    await page.getByRole('textbox', { name: 'Ortsname', exact: true }).fill('Bonn');
    await page.getByRole('button', { name: 'Gebiete suchen', exact: true }).click();
    await expect(page.getByRole('heading', { name: /Gefundene Gebiete/ })).toBeVisible({
      timeout: 90000,
    });
    await page.getByRole('button', { name: 'Bericht exportieren' }).click();
    const live = JSON.parse(await page.getByRole('dialog').locator('pre').innerText());
    expect(live.operation).toBe('areas');
    expect(live.environment).toBe('TEST');
    expect(live.data.areas.length).toBeGreaterThan(0);
    expect(live.requests.length).toBeGreaterThan(0);
    for (const request of live.requests) {
      expect(request.method).toBe('GET');
      expect(new URL(request.url).hostname).toBe('routing-api-testing.fit-connect.fitko.dev');
      expect([200, 301, 302, 303, 307, 308]).toContain(request.status);
    }
    expect(live.requests.some((request) => request.status === 200)).toBe(true);
    expect(live.rawResponses).toBeUndefined();
    await writeFile(join(output, 'live-test-areas.json'), JSON.stringify(live, null, 2));
    await page.keyboard.press('Escape');
    await page.screenshot({ path: join(output, 'windows-live-test-areas.png') });
    results.push(
      'Native UI → bundled Java → public TEST area search returned real results over read-only GET',
    );
  }
  if (errors.length)
    throw new Error('Native renderer reported runtime errors: ' + errors.join('\n'));
  const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
  await writeFile(
    join(output, 'native-windows-results.json'),
    JSON.stringify(
      {
        ok: true,
        at: new Date().toISOString(),
        sourceCommit: process.env.CANDIDATE_SOURCE_COMMIT || process.env.GITHUB_SHA || null,
        harnessCommit: process.env.GITHUB_SHA || null,
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
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  await writeFile(
    join(output, 'native-windows-failure.json'),
    JSON.stringify(
      {
        ok: false,
        at: new Date().toISOString(),
        message,
        startupError,
        results,
        sourceCommit: process.env.CANDIDATE_SOURCE_COMMIT || process.env.GITHUB_SHA || null,
        harnessCommit: process.env.GITHUB_SHA || null,
      },
      null,
      2,
    ),
  );
  const annotation = message
    .slice(0, 1800)
    .replaceAll('%', '%25')
    .replaceAll('\r', '%0D')
    .replaceAll('\n', '%0A');
  console.error(`::error title=Native WebView2 verification::${annotation}`);
  throw error;
} finally {
  try {
    if (browser) await browser.close();
  } finally {
    child.kill();
    if (hosted) testPolicy('disable');
  }
}
