import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { join, resolve } from 'node:path';

test(
  'Windows PowerShell reads the actual NVDA control labels without mojibake',
  { skip: process.platform !== 'win32' },
  () => {
    const result = spawnSync(
      join(process.env.SystemRoot, 'System32/WindowsPowerShell/v1.0/powershell.exe'),
      [
        '-NoProfile',
        '-NonInteractive',
        '-Command',
        '$text = Get-Content -LiteralPath $env:INSPECT_ENCODING_TEST_SOURCE -Raw; [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($text))',
      ],
      {
        encoding: 'utf8',
        timeout: 10000,
        env: {
          ...process.env,
          INSPECT_ENCODING_TEST_SOURCE: resolve('scripts/windows-accessibility-smoke.ps1'),
        },
      },
    );
    assert.equal(result.status, 0, result.stderr);
    const decoded = Buffer.from(result.stdout.trim(), 'base64').toString('utf8');
    assert.ok(
      decoded.includes("'Schlüssel prüfen'"),
      'PowerShell must find the same localized control names as the application',
    );
  },
);
