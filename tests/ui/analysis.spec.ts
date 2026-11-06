import { test, expect } from './fixtures';
import { selectAppearance } from './theme';
const destination = '00000000-0000-4000-8000-000000000001';
const finding = {
  status: 'unknown',
  title: 'Synthetischer Serverbefund',
  detail: 'Kein TLS-Vertrauen bestätigt.',
  path: '$',
  source: 'https://example.org/',
  nextStep: 'HTTPS-CA prüfen.',
  limitation: 'Lokaler Testbeleg.',
};
const report = {
  protocolVersion: 1,
  appVersion: '0.2.0',
  createdAt: '2026-11-06T12:00:00Z',
  operation: 'network',
  environment: 'TEST',
  apiVersion: 'v3',
  findings: [finding],
  requests: [],
  data: {},
};
const result = {
  format: 'fbforge-inspect-result',
  formatVersion: 1,
  appVersion: '0.2.0',
  createdAt: '2026-11-06T12:00:00Z',
  runtime: { os: 'Linux', architecture: 'aarch64', javaVersion: '25' },
  checks: [{ id: 'server-network', report }],
  exitCode: 3,
};
async function navigate(page: import('@playwright/test').Page, name: string) {
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name, exact: true })
    .click();
}

test('smart paste previews the environment and transfers context without starting a diagnosis', async ({
  page,
}) => {
  await page.goto('/');
  await navigate(page, 'Werkzeuge');
  await page
    .getByRole('textbox', { name: 'Destination-ID oder öffentliche FIT-Connect-URL' })
    .fill(`https://prod.fit-connect.fitko.net/submission-api/v2/destinations/${destination}`);
  await expect(page.getByText('Erkannt: Zustellpunkt · PROD · v2')).toBeVisible();
  await page.getByRole('button', { name: 'Angaben übernehmen' }).click();
  await expect(page.getByRole('combobox', { name: 'Umgebung', exact: true })).toHaveValue('PROD');
  await expect(page.getByRole('combobox', { name: 'API', exact: true })).toHaveValue('v2');
  await expect(page.getByRole('textbox', { name: 'Destination-ID', exact: true })).toHaveValue(
    destination,
  );
  await expect(page.getByRole('heading', { name: 'Bereit für Ihre Prüfung' })).toBeVisible();
});

test('batch preparation remains explicit and rejects mixed environments', async ({ page }) => {
  await page.goto('/');
  await navigate(page, 'Sammelprüfung');
  await page.getByRole('textbox', { name: 'Prüfeingaben' }).fill(`${destination}\nnetwork`);
  await page.getByRole('button', { name: 'Prüfungen vorbereiten' }).click();
  await expect(page.getByRole('heading', { name: 'Prüfwarteschlange · TEST · v3' })).toBeVisible();
  await expect(page.getByText('Noch nicht ausgeführt', { exact: true })).toHaveCount(2);
  await page
    .getByRole('textbox', { name: 'Prüfeingaben' })
    .fill(`https://prod.fit-connect.fitko.net/submission-api/v3/destinations/${destination}`);
  await page.getByRole('button', { name: 'Prüfungen vorbereiten' }).click();
  await expect(page.getByRole('alert')).toContainText('Die eingefügte URL gehört zu PROD');
});

test('batch runs public requests sequentially and permits a single targeted repeat', async ({
  page,
}) => {
  await page.addInitScript((fixture) => {
    Object.assign(window, {
      isTauri: true,
      __inspectCalls: [],
      __TAURI_INTERNALS__: {
        invoke: async (command: string, args: { request?: { operation: string } }) => {
          if (command !== 'diagnose') return false;
          (window as unknown as { __inspectCalls: string[] }).__inspectCalls.push(
            args.request!.operation,
          );
          return { ok: true, report: { ...fixture, operation: args.request!.operation } };
        },
      },
    });
  }, report);
  await page.goto('/');
  await navigate(page, 'Sammelprüfung');
  await page.getByRole('textbox', { name: 'Prüfeingaben' }).fill(`${destination}\nnetwork`);
  await page.getByRole('button', { name: 'Prüfungen vorbereiten' }).click();
  await page.getByRole('button', { name: 'Sammelprüfung starten', exact: true }).click();
  await expect(page.getByText('Sammelprüfung abgeschlossen.', { exact: true })).toBeVisible();
  expect(
    await page.evaluate(() => (window as unknown as { __inspectCalls: string[] }).__inspectCalls),
  ).toEqual(['destination', 'network']);
  await page.getByRole('button', { name: 'Diese Prüfung wiederholen' }).nth(1).click();
  await expect
    .poll(() =>
      page.evaluate(() => (window as unknown as { __inspectCalls: string[] }).__inspectCalls),
    )
    .toEqual(['destination', 'network', 'network']);
});

test('Linux server result import is local and malformed findings are rejected', async ({
  page,
}) => {
  await page.goto('/');
  await navigate(page, 'Serverdiagnose');
  const file = page.getByLabel('Serverergebnis öffnen', { exact: true });
  await file.setInputFiles({
    name: 'server.inspect.json',
    mimeType: 'application/json',
    buffer: Buffer.from(JSON.stringify(result)),
  });
  await expect(page.getByText('Linux · aarch64 · Java 25', { exact: true })).toBeVisible();
  await expect(
    page.getByRole('heading', { name: 'Synthetischer Serverbefund · Nicht nachgewiesen' }),
  ).toBeVisible();
  await expect(
    page.getByText('Serverergebnis geöffnet. Es wurde keine Verbindung gestartet.'),
  ).toBeVisible();
  const invalid = structuredClone(result);
  (invalid.checks[0]!.report.findings[0] as unknown as { title: unknown }).title = {};
  await file.setInputFiles({
    name: 'invalid.json',
    mimeType: 'application/json',
    buffer: Buffer.from(JSON.stringify(invalid)),
  });
  await expect(page.getByRole('alert')).toContainText('Textfeld ungültig');
});

test('new workflows reflow in both appearances and retain entered tool values', async ({
  page,
}) => {
  await page.setViewportSize({ width: 700, height: 540 });
  await page.goto('/');
  for (const appearance of ['light', 'dark']) {
    await selectAppearance(page, appearance);
    for (const name of [
      'Sammelprüfung',
      'Werkzeuge',
      'Zertifikatsinventar',
      'Serverdiagnose',
      'Hilfe',
      'Info',
    ]) {
      await navigate(page, name);
      expect(
        await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1),
      ).toBeTruthy();
    }
  }
  await navigate(page, 'Werkzeuge');
  await page.getByRole('combobox', { name: 'Aufgabe', exact: true }).selectOption('reverse');
  await page.getByRole('textbox', { name: 'Destination-ID', exact: true }).fill(destination);
  await navigate(page, 'Serverdiagnose');
  await navigate(page, 'Werkzeuge');
  await expect(page.getByRole('textbox', { name: 'Destination-ID', exact: true })).toHaveValue(
    destination,
  );
  await page.screenshot({
    path: '.local/quality/screenshots/0.2.0-tools-compact-dark.png',
    fullPage: true,
  });
});

async function nativeBridge(page: import('@playwright/test').Page, fixture = report) {
  await page.addInitScript((report) => {
    Object.assign(window, {
      isTauri: true,
      __inspectCalls: [],
      __saved: [],
      __failSave: false,
      __TAURI_INTERNALS__: {
        invoke: async (command: string, args: any) => {
          const w = window as any;
          if (command === 'save_report') {
            if (w.__failSave) throw 'synthetic_write_failed';
            w.__saved.push(args);
            return true;
          }
          if (command === 'diagnose') {
            w.__inspectCalls.push(args.request);
            return { ok: true, report: { ...report, operation: args.request.operation } };
          }
          if (command === 'export_cli') return true;
          return false;
        },
      },
    });
  }, fixture);
}

test('public analysis tools submit explicit paired contexts and clear stale results on edits', async ({
  page,
}) => {
  await nativeBridge(page);
  await page.goto('/');
  await navigate(page, 'Werkzeuge');
  const task = page.getByRole('combobox', { name: 'Aufgabe', exact: true });
  await task.selectOption('reverse');
  await page.getByLabel('Destination-ID', { exact: true }).fill(destination);
  await page.getByLabel('LeiKa-Schlüssel (optional)', { exact: true }).fill('99008001014002');
  await page.getByRole('button', { name: 'Rückwärtsprüfung starten' }).click();
  await expect(page.getByRole('alert')).toContainText('gemeinsam angeben');
  expect(await page.evaluate(() => (window as any).__inspectCalls.length)).toBe(0);
  await page.getByLabel('Vollständiger ARS (optional)', { exact: true }).fill('020000000000');
  await page.getByRole('button', { name: 'Rückwärtsprüfung starten' }).click();
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toBeVisible();
  await page.getByLabel('Vollständiger ARS (optional)', { exact: true }).fill('030000000000');
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toHaveCount(0);
  await task.selectOption('rollover');
  await page.getByLabel('Neue veröffentlichte Schlüsselkennung').fill('replacement-key');
  await page.getByLabel('Öffentlichen Sperrstatus prüfen').uncheck();
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toBeVisible();
  await task.selectOption('signature');
  await page
    .getByLabel('Öffentliche Adressierungssignatur', { exact: true })
    .fill('synthetic.public.signature');
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toBeVisible();
  await task.selectOption('certificate');
  await page.getByLabel('Öffentliches Zertifikat auswählen').setInputFiles({
    name: 'public.der',
    mimeType: 'application/pkix-cert',
    buffer: Buffer.from([0x30, 0x03, 0x01]),
  });
  await expect(page.getByLabel('Öffentliche PEM-Kette oder Base64-DER')).toHaveValue('MAMB');
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toBeVisible();
  const calls = await page.evaluate(() => (window as any).__inspectCalls);
  expect(calls.map((call: any) => call.operation)).toEqual([
    'reverse',
    'rollover',
    'signature',
    'certificate',
  ]);
  expect(calls[0].input.pairs[0].region).toBe('020000000000');
  expect(calls[1].input).toMatchObject({ candidateKid: 'replacement-key', revocation: false });
  await page.getByLabel('Öffentliches Zertifikat auswählen').setInputFiles({
    name: 'private.pem',
    mimeType: 'text/plain',
    buffer: Buffer.from('-----BEGIN PRIVATE KEY-----'),
  });
  await expect(page.getByRole('alert')).toContainText('Private Schlüssel');
  await task.selectOption('error');
  for (const [code, text] of [
    [407, 'Proxy'],
    [403, 'zurückgewiesen'],
    [404, 'kein Eintrag'],
    [429, 'Ratenlimit'],
    [503, 'Serverfehler'],
    [200, 'erfolgreich'],
    [301, 'Kontext'],
  ] as const) {
    await page.getByLabel('HTTP-Status', { exact: true }).fill(String(code));
    await expect(page.locator('main').getByRole('status')).toContainText(text);
  }
});

test('imported proxy settings require adoption and server job export never contains credentials', async ({
  page,
}) => {
  await nativeBridge(page);
  await page.goto('/');
  await navigate(page, 'Sammelprüfung');
  const job = {
    format: 'fbforge-inspect-job',
    formatVersion: 1,
    environment: 'PROD',
    apiVersion: 'v2',
    network: { mode: 'manual', proxyUrl: 'https://proxy.example:8443' },
    checks: [{ id: 'network', operation: 'network', input: {} }],
  };
  await page.getByLabel('Prüfauftrag öffnen', { exact: true }).setInputFiles({
    name: 'proxy-job.json',
    mimeType: 'application/json',
    buffer: Buffer.from(JSON.stringify(job)),
  });
  await expect(page.getByRole('heading', { name: 'Verbindungsweg aus dem Auftrag' })).toBeVisible();
  expect(await page.evaluate(() => (window as any).__inspectCalls.length)).toBe(0);
  await page.getByRole('button', { name: 'Verbindungsweg und Auftrag übernehmen' }).click();
  await expect(page.getByRole('combobox', { name: 'Umgebung', exact: true })).toHaveValue('PROD');
  await expect(page.getByRole('combobox', { name: 'API', exact: true })).toHaveValue('v2');
  await page.getByRole('button', { name: 'Auftrag für Server speichern' }).click();
  await expect.poll(() => page.evaluate(() => (window as any).__saved.length)).toBe(1);
  const saved = await page.evaluate(() => JSON.parse((window as any).__saved[0].content));
  expect(saved.network).toEqual(job.network);
  expect(saved.checks).toEqual(job.checks);
  await page.getByRole('button', { name: 'Sammelprüfung starten', exact: true }).click();
  await expect(page.getByText('Sammelprüfung abgeschlossen.', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Ergebnisse speichern', exact: true }).click();
  await expect.poll(() => page.evaluate(() => (window as any).__saved.length)).toBe(2);
  expect(await page.evaluate(() => JSON.parse((window as any).__saved[1].content).format)).toBe(
    'fbforge-inspect-result',
  );
  await page.evaluate(() => ((window as any).__failSave = true));
  await page.getByRole('button', { name: 'Ergebnisse speichern', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('nicht gespeichert');
});

test('inventory groups affected destinations and exports a calendar, support stays a preview', async ({
  page,
}) => {
  await nativeBridge(page);
  await page.goto('/');
  const evidence = structuredClone(result);
  evidence.checks[0]!.report.data = {
    destination: { destinationId: destination, encryptionKid: 'active-key' },
    certificates: [
      {
        chainIndex: 0,
        sha256: 'a'.repeat(64),
        kid: 'active-key',
        subject: 'CN=Fiktive Teilnehmerstelle',
        issuer: 'CN=Fiktive CA',
        notBefore: '2026-01-01T00:00:00Z',
        notAfter: '2027-01-01T00:00:00Z',
      },
    ],
  };
  await navigate(page, 'Serverdiagnose');
  await page.getByLabel('Serverergebnis öffnen', { exact: true }).setInputFiles({
    name: 'server.json',
    mimeType: 'application/json',
    buffer: Buffer.from(JSON.stringify(evidence)),
  });
  await page.getByText('Supportanfrage vorbereiten', { exact: true }).click();
  const preview = page
    .locator('details')
    .filter({ has: page.getByText('Supportanfrage vorbereiten', { exact: true }) });
  await expect(preview.locator('pre')).not.toContainText('https://example.org/');
  await page.getByLabel('Quelladressen und Feldpfade aufnehmen').check();
  await expect(preview.locator('pre')).toContainText('https://example.org/');
  await page.getByRole('combobox', { name: /^Ansprechstelle/ }).selectOption('fitko');
  await expect(preview.locator('pre')).toContainText('Diagnose für FITKO-Support');
  await page.getByRole('button', { name: 'Supporttext speichern', exact: true }).click();
  await expect(page.getByText('Supporttext gespeichert.', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Portables CLI exportieren', exact: true }).click();
  await expect(page.getByRole('status').first()).toContainText('exportiert');
  await navigate(page, 'Zertifikatsinventar');
  await expect(page.getByRole('heading', { name: 'CN=Fiktive Teilnehmerstelle' })).toBeVisible();
  await page.getByLabel('Erinnerung vor Ablauf').selectOption('30');
  await page.getByRole('button', { name: 'Erneuerungstermine exportieren' }).click();
  await expect(page.getByText('Erneuerungstermine gespeichert.', { exact: true })).toBeVisible();
  expect(await page.evaluate(() => (window as any).__saved.at(-1))).toMatchObject({
    format: 'ics',
    content: expect.stringContaining('DTSTART;VALUE=DATE:20261202'),
  });
  await page.getByRole('button', { name: 'Zustellpunkt vorbereiten', exact: true }).click();
  await expect(page.getByLabel('Destination-ID', { exact: true })).toHaveValue(destination);
  await expect(page.getByRole('heading', { name: 'Bereit für Ihre Prüfung' })).toBeVisible();
});

test('updater is explicit, recovers from a failed signature and escapes release notes', async ({
  page,
}) => {
  await page.addInitScript(() => {
    const w = window as any;
    const status = {
      configured: true,
      supported: true,
      phase: 'idle',
      version: null,
      notes: '',
      downloaded: 0,
      total: null,
    };
    Object.assign(w, {
      isTauri: true,
      __updates: [],
      __failDownload: true,
      __TAURI_INTERNALS__: {
        invoke: async (command: string) => {
          if (command === 'application_update_status') return { ...status };
          w.__updates.push(command);
          if (command === 'check_application_update')
            Object.assign(status, {
              phase: 'available',
              version: '0.3.0',
              notes: '<script>untrusted release notes</script>',
            });
          if (command === 'download_application_update') {
            if (w.__failDownload) throw 'update_signature_invalid';
            status.phase = 'downloaded';
          }
          if (command === 'install_application_update') throw 'update_activity_pending';
          return { ...status };
        },
      },
    });
  });
  await page.goto('/');
  await navigate(page, 'Info');
  expect(await page.evaluate(() => (window as any).__updates)).toEqual([]);
  await page.getByRole('button', { name: 'Nach Updates suchen', exact: true }).click();
  await expect(page.locator('pre').filter({ hasText: 'untrusted release notes' })).toHaveText(
    '<script>untrusted release notes</script>',
  );
  expect(await page.locator('pre script').count()).toBe(0);
  await page.getByRole('button', { name: 'Version 0.3.0 herunterladen' }).click();
  await expect(page.getByRole('alert')).toContainText('Updatesignatur ist ungültig');
  await expect(page.getByRole('button', { name: 'Geprüftes Update installieren' })).toHaveCount(0);
  await page.evaluate(() => ((window as any).__failDownload = false));
  await page.getByRole('button', { name: 'Version 0.3.0 herunterladen' }).click();
  await page.getByRole('button', { name: 'Geprüftes Update installieren' }).click();
  await expect(page.getByRole('alert')).toContainText('Beenden Sie laufende Prüfungen');
});

test('batch pause waits for the active check and cancellation retains completed results', async ({
  page,
}) => {
  await page.addInitScript((fixture) => {
    const w = window as any;
    Object.assign(w, {
      isTauri: true,
      __started: 0,
      __pending: null,
      __TAURI_INTERNALS__: {
        invoke: (command: string) => {
          if (command === 'diagnose') {
            w.__started++;
            return new Promise((resolve) => {
              w.__pending = () => resolve({ ok: true, report: fixture });
            });
          }
          if (command === 'cancel') {
            w.__pending();
            return Promise.resolve(true);
          }
          return Promise.resolve(false);
        },
      },
    });
  }, report);
  await page.goto('/');
  await navigate(page, 'Sammelprüfung');
  await page.getByLabel('Prüfeingaben').fill(`${destination}\nnetwork`);
  await page.getByRole('button', { name: 'Prüfungen vorbereiten' }).click();
  await page.getByRole('button', { name: 'Sammelprüfung starten', exact: true }).click();
  await expect.poll(() => page.evaluate(() => (window as any).__started)).toBe(1);
  await page.getByRole('button', { name: 'Nach aktueller Prüfung pausieren' }).click();
  await page.evaluate(() => (window as any).__pending());
  await expect(page.getByText('Abgeschlossen', { exact: true })).toHaveCount(1);
  expect(await page.evaluate(() => (window as any).__started)).toBe(1);
  await page.getByRole('button', { name: 'Fortsetzen', exact: true }).click();
  await expect.poll(() => page.evaluate(() => (window as any).__started)).toBe(2);
  await page.getByRole('button', { name: 'Sammelprüfung abbrechen', exact: true }).click();
  await expect(
    page.getByText('Sammelprüfung abgebrochen. Abgeschlossene Ergebnisse bleiben erhalten.', {
      exact: true,
    }),
  ).toBeVisible();
  await expect(page.getByText('Abgeschlossen', { exact: true })).toHaveCount(1);
  await expect(page.getByText('Noch nicht ausgeführt', { exact: true })).toHaveCount(1);
});

test('failed native cancellation is explained and later completion remains usable', async ({
  page,
}) => {
  await page.addInitScript((fixture) => {
    const w = window as any;
    Object.assign(w, {
      isTauri: true,
      __pending: null,
      __TAURI_INTERNALS__: {
        invoke: (command: string) => {
          if (command === 'diagnose')
            return new Promise(
              (resolve) => (w.__pending = () => resolve({ ok: true, report: fixture })),
            );
          if (command === 'cancel') return Promise.reject('synthetic_cancel_failure');
          return Promise.resolve(false);
        },
      },
    });
  }, report);
  await page.goto('/');
  await navigate(page, 'Sammelprüfung');
  await page.getByLabel('Prüfeingaben').fill('network');
  await page.getByRole('button', { name: 'Prüfungen vorbereiten' }).click();
  await page.getByRole('button', { name: 'Sammelprüfung starten', exact: true }).click();
  await page.getByRole('button', { name: 'Sammelprüfung abbrechen', exact: true }).click();
  await expect(
    page.getByText('Abbruch nicht bestätigt. Die aktuelle Prüfung wird weiter abgewartet.', {
      exact: true,
    }),
  ).toBeVisible();
  await page.evaluate(() => (window as any).__pending());
  await expect(page.getByText('Sammelprüfung abgeschlossen.', { exact: true })).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Diese Prüfung wiederholen', exact: true }),
  ).toBeEnabled();
});

test('transferred proxy and target TLS evidence remain separate and offer no automatic retries', async ({
  page,
}) => {
  const evidence = structuredClone(result);
  const peer = (subject: string) => ({
    protocol: 'TLSv1.3',
    cipher: 'TLS_AES_256_GCM_SHA384',
    certificates: [
      {
        subject,
        issuer: 'CN=Fiktive CA',
        notAfter: '2027-01-01T00:00:00Z',
        sha256: 'a'.repeat(64),
      },
    ],
  });
  evidence.checks[0]!.report.data = {
    networkContext: { httpsTrust: 'Java und ausdrücklich geladene Unternehmens-CA' },
    networkChecks: [
      {
        name: 'Fiktives FIT-Connect-Ziel',
        host: 'example.org',
        port: 443,
        url: 'https://example.org/',
        method: 'GET',
        detail: 'Testnachweis',
        code: 'HTTP_OK',
        httpStatus: 200,
        durationMs: 25,
      },
    ],
    networkTraces: [
      {
        url: 'https://example.org/',
        route: 'manualProxy',
        phases: [
          { stage: 'proxyTls', durationMs: 5, outcome: 'confirmed' },
          { stage: 'tls', durationMs: 10, outcome: 'confirmed' },
          { stage: 'http', outcome: 'TLS_FAILED' },
        ],
        proxyTls: peer('CN=Fiktiver HTTPS-Proxy'),
        tls: peer('CN=Fiktiver Zielserver'),
      },
    ],
  };
  await page.goto('/');
  await navigate(page, 'Serverdiagnose');
  await page.getByLabel('Serverergebnis öffnen', { exact: true }).setInputFiles({
    name: 'network.json',
    mimeType: 'application/json',
    buffer: Buffer.from(JSON.stringify(evidence)),
  });
  await page.getByText('Verbindungsphasen und HTTPS-Vertrauen', { exact: true }).click();
  await page.getByText('Vorgelegtes Zertifikat des HTTPS-Proxys', { exact: true }).click();
  await page.getByText('Vorgelegtes HTTPS-Serverzertifikat', { exact: true }).click();
  await expect(page.getByText('CN=Fiktiver HTTPS-Proxy', { exact: true })).toBeVisible();
  await expect(page.getByText('CN=Fiktiver Zielserver', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Dieses Ziel erneut prüfen' })).toHaveCount(0);
  await page.getByText('Supportanfrage vorbereiten', { exact: true }).click();
  await page.getByLabel('Quelladressen und Feldpfade aufnehmen').check();
  await expect(page.locator('pre').filter({ hasText: 'Diagnose für Lokale IT' })).toContainText(
    'HTTPS-Vertrauen: Java',
  );
  await expect(page.locator('pre').filter({ hasText: 'Diagnose für Lokale IT' })).toContainText(
    'proxyTls: confirmed · 5 ms',
  );
});

test('addressing context exploration keeps authorities separate and follow-up actions only prepare inputs', async ({
  page,
}) => {
  const contextReport = {
    ...report,
    operation: 'routing',
    data: {
      routes: [{ destinationId: destination }],
      pvogRelations: [
        {
          uebergeordnetesObjektID: { value: 'OE-TEST-A' },
          zustaendigkeit: [
            { leistungID: [{ value: 'LB-TEST' }], gebietID: [{ value: 'ARS-TEST' }] },
          ],
        },
        { uebergeordnetesObjektID: { value: 'OE-TEST-B' }, zustaendigkeit: [{}] },
      ],
      addressingSignatures: [
        {
          context: '$.pvogRelations[0].zustaendigkeit[0].idSekundaer[0]',
          verified: true,
          contextMatches: true,
          claims: { destinationId: destination },
        },
      ],
    },
  };
  await nativeBridge(page, contextReport);
  await page.goto('/');
  await navigate(page, 'Routing prüfen');
  await page.getByLabel('LeiKa-Schlüssel', { exact: true }).fill('99008001014002');
  await page.getByLabel('Gebietsschlüssel', { exact: true }).fill('020000000000');
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await page.getByText('Zusammenhänge der Adressierung nachvollziehen', { exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Übergeordnetes PVOG-Objekt · OE-TEST-A' }),
  ).toBeVisible();
  await expect(
    page.getByRole('heading', { name: 'Übergeordnetes PVOG-Objekt · OE-TEST-B' }),
  ).toBeVisible();
  await page
    .getByRole('searchbox', { name: 'Adressierungskontexte durchsuchen' })
    .fill('OE-TEST-A');
  await expect(
    page.getByRole('heading', { name: 'Übergeordnetes PVOG-Objekt · OE-TEST-B' }),
  ).toHaveCount(0);
  await page.getByRole('button', { name: 'Diesen Zustellpunkt vorbereiten', exact: true }).click();
  await expect(page.getByLabel('Destination-ID', { exact: true })).toHaveValue(destination);
  expect(await page.evaluate(() => (window as any).__inspectCalls.length)).toBe(1);
});
