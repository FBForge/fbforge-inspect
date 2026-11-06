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
  await expect(page.getByRole('heading', { name: 'Start', exact: true })).toBeVisible({
    timeout: 20000,
  });
  results.push('Native WebView2 frontend loaded');
  await page.screenshot({ path: join(output, 'windows-start.png') });
  if (process.env.GITHUB_SHA) {
    await page
      .getByRole('navigation', { name: 'Hauptnavigation' })
      .getByRole('button', { name: 'Info', exact: true })
      .click();
    await expect(
      page.getByText(`Build: ${process.env.CANDIDATE_SOURCE_COMMIT || process.env.GITHUB_SHA}`, {
        exact: true,
      }),
    ).toBeVisible();
    await page.screenshot({ path: join(output, 'windows-build-info.png') });
    results.push('Native Info build identity matches the requested source commit');
  }
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
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Routing prüfen', exact: true })
    .click();
  await page.getByRole('textbox', { name: 'LeiKa-Schlüssel', exact: true }).fill('99150085001000');
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Fachdatenschemata', exact: true })
    .click();
  await expect(page.getByRole('textbox', { name: 'JSON Schema / XSD', exact: true })).toHaveValue(
    '{"type":"object","properties":{"d":{"type":"string"}}}',
  );
  await expect(page.getByRole('heading', { name: 'Schema eingelesen', exact: true })).toBeVisible();
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Routing prüfen', exact: true })
    .click();
  await expect(page.getByRole('textbox', { name: 'LeiKa-Schlüssel', exact: true })).toHaveValue(
    '99150085001000',
  );
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Fachdatenschemata', exact: true })
    .click();
  results.push('Native workspace navigation preserves separate inputs and completed reports');
  await page.locator('main').evaluate((element) => (element.scrollTop = element.scrollHeight));
  expect(await page.locator('main').evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Routing prüfen', exact: true })
    .click();
  await expect.poll(() => page.locator('main').evaluate((element) => element.scrollTop)).toBe(0);
  results.push('Native navigation restores the target workspace scroll position');
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Fachdatenschemata', exact: true })
    .click();
  const originalSchema =
    '{"type":"object","properties":{"d":{"type":"string"}},"$ref":"https://example.org/not-fetched"}';
  await page.getByRole('textbox', { name: 'JSON Schema / XSD', exact: true }).fill(originalSchema);
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await page.getByRole('button', { name: 'Referenz vorbereiten', exact: true }).click();
  await expect(
    page.getByRole('textbox', { name: 'Öffentliche HTTPS-URL', exact: true }),
  ).toHaveValue('https://example.org/not-fetched');
  await page.getByRole('button', { name: 'Vorheriges Schema', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'JSON Schema / XSD', exact: true })).toHaveValue(
    originalSchema,
  );
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toBeVisible();
  await page
    .getByRole('searchbox', { name: 'Struktur durchsuchen', exact: true })
    .fill('properties.d');
  await expect(
    page.getByRole('region', { name: 'Strukturtreffer', exact: true }).locator('tbody tr'),
  ).toHaveCount(1);
  results.push(
    'Native schema reference navigation retains the original input and report; structure search filters fields',
  );

  await page.getByRole('button', { name: 'Bericht exportieren' }).click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await expect(
    page
      .getByRole('dialog')
      .frameLocator('iframe')
      .getByRole('heading', { name: 'FBForge Inspect', exact: true }),
  ).toBeVisible();
  await page
    .getByRole('dialog')
    .getByRole('combobox', { name: 'Format', exact: true })
    .selectOption('json');
  await expect(page.getByRole('dialog').locator('pre')).not.toContainText('rawResponses');
  await page
    .getByRole('dialog')
    .getByRole('combobox', { name: 'Format', exact: true })
    .selectOption('html');
  await page
    .getByRole('dialog')
    .frameLocator('iframe')
    .getByText('Beleg und Feldpfad', { exact: true })
    .first()
    .focus();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toHaveCount(0);
  results.push(
    'Native HTML preview closes with Escape from inside the report and restores its trigger',
  );
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
        .selectOption('json');
      await exportDialog
        .getByRole('checkbox', { name: 'Rohantworten einschließen' })
        .setChecked(raw);
      const preview = JSON.parse(await exportDialog.locator('pre').innerText());
      if (raw) expect(preview).toHaveProperty('rawResponses');
      else expect(preview.rawResponses).toBeUndefined();
      await exportDialog
        .getByRole('combobox', { name: 'Format', exact: true })
        .selectOption(format);
      if (format === 'html')
        await expect(
          exportDialog
            .frameLocator('iframe')
            .getByRole('heading', { name: 'FBForge Inspect', exact: true }),
        ).toBeVisible();
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
  await page.getByRole('button', { name: 'Darstellung wählen', exact: true }).click();
  await page
    .getByRole('group', { name: 'Darstellung', exact: true })
    .getByRole('button', { name: 'Dunkel', exact: true })
    .click();
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
  const navigate = (name) =>
    page
      .getByRole('navigation', { name: 'Hauptnavigation' })
      .getByRole('button', { name, exact: true })
      .click();
  const job = {
    format: 'fbforge-inspect-job',
    formatVersion: 1,
    environment: 'TEST',
    apiVersion: 'v3',
    checks: [
      {
        id: 'native-local-schema',
        operation: 'schema',
        input: {
          text: '{"type":"object","properties":{"native":{"type":"string"}}}',
        },
      },
    ],
  };
  await navigate('Sammelprüfung');
  await page.getByText('Mehrere Kennungen einfügen und Aufträge öffnen', { exact: true }).click();
  await page.getByLabel('Prüfauftrag öffnen', { exact: true }).setInputFiles({
    name: 'native-job.json',
    mimeType: 'application/json',
    buffer: Buffer.from(JSON.stringify(job)),
  });
  await expect(page.getByRole('heading', { name: 'Prüfwarteschlange · TEST · v3' })).toBeVisible();
  await page.getByRole('button', { name: 'Sammelprüfung starten', exact: true }).click();
  await expect(page.getByText('Geprüfte Einzelpunkte bestätigt', { exact: true })).toBeVisible({
    timeout: 15000,
  });
  await page.getByText('Befunde anzeigen', { exact: true }).click();
  await expect(page.getByText('Schema eingelesen', { exact: true })).toBeVisible();
  await page.screenshot({ path: join(output, 'windows-batch.png') });
  results.push('Native batch import and sequential bundled engine execution passed');
  const jobPath = join(profile, 'native-job.json');
  const resultPath = join(profile, 'native-result.json');
  const jobBytes = JSON.stringify(job);
  await writeFile(jobPath, jobBytes);
  const cli = await promisify(execFile)(
    join(dirname(executable), 'engine/runtime/bin/java.exe'),
    [
      '-jar',
      join(dirname(executable), 'engine/inspect-engine.jar'),
      '--job',
      jobPath,
      '--output',
      resultPath,
    ],
    { encoding: 'utf8', timeout: 20000, windowsHide: true },
  );
  const serverResult = JSON.parse(await readFile(resultPath, 'utf8'));
  expect(serverResult.format).toBe('fbforge-inspect-result');
  expect(serverResult.runtime.os).toContain('Windows');
  expect(serverResult.jobSha256).toBe(createHash('sha256').update(jobBytes).digest('hex'));
  expect(serverResult.exitCode).toBe(0);
  expect(serverResult.checks[0].report.rawResponses).toBeUndefined();
  await writeFile(join(output, 'native-cli-stdout.txt'), cli.stdout);
  await navigate('Serverdiagnose');
  await page.getByLabel('Serverergebnis öffnen', { exact: true }).setInputFiles(resultPath);
  await expect(page.getByRole('heading', { name: 'Übertragenes Serverergebnis' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Schema eingelesen · Bestätigt' })).toBeVisible();
  await expect(
    page.getByText('Auftrag SHA-256: ' + serverResult.jobSha256, { exact: true }),
  ).toBeVisible();
  await page
    .getByLabel('Auftragsdatei zur Zuordnung öffnen', { exact: true })
    .setInputFiles(jobPath);
  await expect(
    page.getByText('Ergebnis passt zu den Bytes der ausgewählten Auftragsdatei.', { exact: true }),
  ).toBeVisible();
  await page.screenshot({ path: join(output, 'windows-server-result.png') });
  results.push(
    'Real packaged Java CLI result reopens in native Inspect with matching job digest and findings',
  );
  await navigate('Hilfe');
  await page.getByRole('searchbox', { name: 'Hilfe durchsuchen', exact: true }).fill('CLI');
  const cliHelp = page.locator('summary').filter({ hasText: /^CLI auf Windows und Linux$/ });
  await expect(cliHelp).toBeVisible();
  await cliHelp.click();
  await expect(page.getByText(/^Unter Windows dient inspect\.ps1 als Startskript/)).toBeVisible();
  results.push('Native offline help covers server CLI workflow');
  await navigate('Info');
  await expect(
    page.getByRole('heading', { name: 'Inspect aktualisieren', exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Nach Updates suchen', exact: true }),
  ).toBeEnabled();
  results.push('Native updater is configured and stays idle until an explicit check');
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
    await page
      .getByRole('dialog')
      .getByRole('combobox', { name: 'Format', exact: true })
      .selectOption('json');
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
    await page
      .getByRole('navigation', { name: 'Hauptnavigation' })
      .getByRole('button', { name: 'Destination prüfen', exact: true })
      .click();
    await page
      .getByRole('textbox', { name: 'Destination-ID', exact: true })
      .fill('b21c51d0-324b-4df4-ae6c-c0568a2bb067');
    await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
    await expect(
      page.getByRole('heading', { name: 'Öffentliche Schlüssel · JWK / JWKS', exact: true }),
    ).toBeVisible({ timeout: 150000 });
    await page.getByRole('button', { name: 'Bericht exportieren', exact: true }).click();
    await page
      .getByRole('dialog')
      .getByRole('combobox', { name: 'Format', exact: true })
      .selectOption('json');
    const jwkReport = JSON.parse(await page.getByRole('dialog').locator('pre').innerText());
    expect(jwkReport.operation).toBe('destination');
    expect(jwkReport.data.keys.length).toBeGreaterThan(0);
    expect(jwkReport.data.keys[0].publicJwk.x5c.length).toBeGreaterThan(0);
    expect(jwkReport.data.certificates[0].pem).toContain('BEGIN CERTIFICATE');
    expect(jwkReport.rawResponses).toBeUndefined();
    expect(jwkReport.findings.some((finding) => finding.status === 'error')).toBe(false);
    if (jwkReport.data.certificates[0].subject === jwkReport.data.certificates[0].issuer) {
      expect(jwkReport.data.certificateTrust[0].trusted).toBe(false);
      expect(
        jwkReport.findings.some(
          (finding) =>
            finding.title === 'TEST-Zertifikat ohne V-PKI-Verankerung' &&
            finding.status === 'warning',
        ),
      ).toBe(true);
      expect(
        jwkReport.findings.some(
          (finding) =>
            finding.title === 'Sperrstatus des TEST-Zertifikats' && finding.status === 'warning',
        ),
      ).toBe(true);
    }

    const ocspSources = new Set(
      jwkReport.findings
        .filter((finding) => finding.title.startsWith('OCSP'))
        .map((finding) => finding.source),
    );
    expect(
      jwkReport.requests.every(
        (request) =>
          request.method === 'GET' || (request.method === 'POST' && ocspSources.has(request.url)),
      ),
    ).toBe(true);
    expect(
      jwkReport.requests.some(
        (request) =>
          request.method === 'GET' &&
          request.url.includes('/destinations/') &&
          request.status === 200,
      ),
    ).toBe(true);
    expect(
      jwkReport.findings.some((f) => /X\.509.*(Key-Usage|Zertifikatsverwendung)/.test(f.title)),
    ).toBe(false);
    await writeFile(join(output, 'live-test-jwk.json'), JSON.stringify(jwkReport, null, 2));
    await page.keyboard.press('Escape');
    await page.getByText('JWK-Felder einschließlich x5c anzeigen', { exact: true }).first().click();
    await page.locator('.key-card').first().scrollIntoViewIfNeeded();
    await page.screenshot({ path: join(output, 'windows-public-jwk.png') });
    results.push(
      'Native TEST destination publishes JWK and x5c without certificate key-usage scoring',
    );
    await page
      .getByRole('navigation', { name: 'Hauptnavigation' })
      .getByRole('button', { name: 'Prozessnachricht prüfen', exact: true })
      .click();
    await page.getByRole('button', { name: 'Limits prüfen', exact: true }).click();
    await page
      .getByRole('textbox', { name: 'Absender-Destination', exact: true })
      .fill('b21c51d0-324b-4df4-ae6c-c0568a2bb067');
    await page
      .getByRole('textbox', { name: 'Empfänger-Destination', exact: true })
      .fill('23bdd840-a823-4d8b-9992-07d04ca907a7');
    await page.getByRole('button', { name: 'Limits abrufen', exact: true }).click();
    await expect(
      page.getByRole('heading', { name: 'Größenlimits für Anlagen', exact: true }),
    ).toBeVisible({ timeout: 30000 });
    await page.getByRole('button', { name: 'Bericht exportieren', exact: true }).click();
    await page
      .getByRole('dialog')
      .getByRole('combobox', { name: 'Format', exact: true })
      .selectOption('json');
    const limitsReport = JSON.parse(await page.getByRole('dialog').locator('pre').innerText());
    expect(limitsReport.operation).toBe('limits');
    expect(limitsReport.data.limits.attachments.applicable.maxNumber).toBeGreaterThan(0);
    expect(
      limitsReport.requests.every(
        (request) => request.method === 'GET' && request.url.includes('/v3/limits/'),
      ),
    ).toBe(true);
    expect(limitsReport.requests.some((request) => request.status === 200)).toBe(true);
    await writeFile(join(output, 'live-test-limits.json'), JSON.stringify(limitsReport, null, 2));
    await page.keyboard.press('Escape');
    await page.screenshot({ path: join(output, 'windows-live-test-limits.png') });
    results.push(
      'Native independent limits request uses only two destinations and returns real public limits',
    );
    await page
      .getByRole('navigation', { name: 'Hauptnavigation' })
      .getByRole('button', { name: 'Destination prüfen', exact: true })
      .click();
    await page.getByRole('combobox', { name: 'Umgebung', exact: true }).selectOption('PROD');
    const productionReports = [];
    for (const version of ['v2', 'v3']) {
      await page.getByRole('combobox', { name: 'API', exact: true }).selectOption(version);
      const destinationId = '9854de30-5547-42b1-b6d2-d7bd80253c49';
      await page.getByRole('textbox', { name: 'Destination-ID', exact: true }).fill(destinationId);
      await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
      await expect(
        page.getByRole('heading', { name: 'Öffentliche Schlüssel · JWK / JWKS', exact: true }),
      ).toBeVisible({ timeout: 150000 });
      await page.getByRole('button', { name: 'Bericht exportieren', exact: true }).click();
      await page
        .getByRole('dialog')
        .getByRole('combobox', { name: 'Format', exact: true })
        .selectOption('json');
      const production = JSON.parse(await page.getByRole('dialog').locator('pre').innerText());
      expect(production.environment).toBe('PROD');
      expect(production.apiVersion).toBe(version);
      const kid = production.data.destination.encryptionKid;
      expect(kid).toContain(' ');
      expect(production.data.keys[0].kid).toBe(kid);
      const keyRequest = production.requests.find((request) => request.url.includes('/keys/'));
      expect(keyRequest?.status).toBe(200);
      expect(keyRequest.url).toContain(
        `/submission-api/${version}/destinations/${destinationId}/keys/`,
      );
      expect(keyRequest.url).toContain('%20');
      expect(keyRequest.url).not.toContain('+');
      expect(new URL(keyRequest.url).pathname.split('/').map(decodeURIComponent).at(-1)).toBe(kid);
      expect(production.rawResponses).toBeUndefined();
      await writeFile(
        join(output, `live-prod-destination-${version}.json`),
        JSON.stringify(production, null, 2),
      );
      await page.keyboard.press('Escape');
      if (
        production.findings.every((finding) =>
          ['confirmed', 'notApplicable'].includes(finding.status),
        )
      ) {
        await expect(
          page.getByRole('heading', { name: 'Geprüfte Konfiguration in Ordnung', exact: true }),
        ).toBeVisible();
        results.push(`Native PROD ${version} positive findings have a matching result summary`);
      }
      await page.screenshot({ path: join(output, `windows-live-prod-destination-${version}.png`) });
      productionReports.push(production);
      results.push(
        `Native PROD ${version} destination resolves a key ID containing spaces; its complete PKI report is retained`,
      );
    }
    // Collect both versions before requiring positive status from the external PKI services.
    // An external outage still fails this acceptance check and remains visible in both reports.
    for (const production of productionReports) {
      expect(production.data.certificateTrust[0].trusted).toBe(true);
      expect(production.data.certificateTrust[0].source).toBe('bundled-vpki');
      expect(
        production.findings.some(
          (finding) => finding.status === 'error' || finding.status === 'unknown',
        ),
      ).toBe(false);
      expect(
        production.findings.some(
          (finding) => finding.title === 'OCSP-Sperrstatus' && finding.status === 'confirmed',
        ),
      ).toBe(true);
      expect(
        production.findings.filter(
          (finding) => finding.title === 'CRL-Sperrstatus' && finding.status === 'confirmed',
        ).length,
      ).toBeGreaterThanOrEqual(2);
    }
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
