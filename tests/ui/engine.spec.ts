// Real Java engine through a test-only browser bridge. This does not claim native Tauri coverage.
import { test, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { join } from 'node:path';
const java = process.env.JAVA_HOME
  ? join(process.env.JAVA_HOME, 'bin', process.platform === 'win32' ? 'java.exe' : 'java')
  : 'java';
test.beforeEach(async ({ page }) => {
  await page.exposeFunction(
    'inspectEngine',
    (request: unknown) =>
      new Promise((resolve, reject) => {
        const child = spawn(java, ['-jar', 'engine/target/inspect-engine-0.1.0.jar']);
        let output = '';
        const timer = setTimeout(() => {
          child.kill();
          reject(new Error('engine timeout'));
        }, 15000);
        child.stdout.on('data', (data) => {
          output += data.toString('utf8');
        });
        child.on('error', (error) => {
          clearTimeout(timer);
          reject(error);
        });
        child.on('close', (code) => {
          clearTimeout(timer);
          if (code) reject(new Error(`engine exit ${code}`));
          else {
            try {
              resolve(JSON.parse(output));
            } catch (error) {
              reject(error);
            }
          }
        });
        child.stdin.end(JSON.stringify(request));
      }),
  );
  await page.addInitScript(() =>
    Object.assign(window, {
      isTauri: true,
      __TAURI_INTERNALS__: {
        invoke: async (command: string, args: any) => {
          if (command !== 'diagnose') throw new Error('Not provided by engine bridge');
          return (window as any).inspectEngine(args.request);
        },
      },
    }),
  );
  await page.goto('/');
});
test('local JSON schema file reaches real engine and never resolves external references', async ({
  page,
}) => {
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Fachschemata', exact: true })
    .click();
  await expect(page.locator('#page-title')).toBeFocused();
  await page.getByLabel('Datei auswählen').setInputFiles({
    name: 'schema.json',
    mimeType: 'application/json',
    buffer: Buffer.from(
      JSON.stringify({
        type: 'object',
        properties: { name: { type: 'string' } },
        $ref: 'https://example.org/not-fetched',
      }),
    ),
  });
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await expect(page.getByText('Schema eingelesen', { exact: true })).toBeVisible();
  await expect(page.getByText('https://example.org/not-fetched', { exact: true })).toBeVisible();
  await page.screenshot({ path: 'test-results/schema-real-engine.png', fullPage: true });
});
test('XSD is inspected and XXE is rejected through the real engine', async ({ page }) => {
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Fachschemata', exact: true })
    .click();
  const input = page.getByRole('textbox', { name: 'JSON Schema / XSD' });
  await input.fill(
    '<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"><xs:element name="example" type="xs:string"/></xs:schema>',
  );
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Diagnoseergebnis', exact: true })).toBeVisible();
  await input.fill(
    '<!DOCTYPE x [<!ENTITY external SYSTEM "file:///etc/passwd">]><x>&external;</x>',
  );
  await page.getByRole('button', { name: 'Prüfung starten', exact: true }).click();
  await expect(page.getByRole('alert')).toBeVisible();
  await expect(page.locator('body')).not.toContainText('root:x:');
});
test('oversized and private files are rejected in the UI', async ({ page }) => {
  await page
    .getByRole('navigation', { name: 'Hauptnavigation' })
    .getByRole('button', { name: 'Schlüssel prüfen', exact: true })
    .click();
  await page.getByLabel('Datei auswählen').setInputFiles({
    name: 'private.json',
    mimeType: 'application/json',
    buffer: Buffer.from('{"kty":"RSA","d":"secret-fixture"}'),
  });
  await expect(page.getByRole('alert')).toContainText('Private oder symmetrische Schlüssel');
  await expect(page.getByRole('textbox', { name: 'Öffentliches JWK / JWKS' })).toHaveValue('');
  await page.getByLabel('Datei auswählen').setInputFiles({
    name: 'big.json',
    mimeType: 'application/json',
    buffer: Buffer.alloc(1500001, 32),
  });
  await expect(page.getByRole('alert')).toContainText('1,5 MB');
});
