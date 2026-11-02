import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, rm, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { powershellRuntime } from './powershell.mjs';

const { executable: shell, environment } = powershellRuntime();
const available =
  spawnSync(shell, ['-NoProfile', '-Command', 'exit 0'], { env: environment }).status === 0;
test(
  'signing rejects invalid keys and signatures and preserves the packaged signed bytes',
  { skip: !available && 'PowerShell unavailable' },
  async () => {
    const directory = await mkdtemp(join(tmpdir(), 'inspect-signing-'));
    try {
      const result = spawnSync(
        shell,
        [
          '-NoProfile',
          '-NonInteractive',
          '-ExecutionPolicy',
          'Bypass',
          '-File',
          'scripts/ci/test-windows-signing.ps1',
          '-TestDirectory',
          directory,
        ],
        { encoding: 'utf8', timeout: 30000, env: environment },
      );
      assert.equal(result.status, 0, result.stdout + result.stderr);
      assert.match(result.stdout, /Signing contract tests passed/);
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  },
);

test('the signer is manual, main-only, approval-gated and never publishes a release', async () => {
  const text = await readFile('.github/workflows/sign-windows.yml', 'utf8');
  assert.match(text, /workflow_dispatch:/);
  assert.match(text, /github\.ref == 'refs\/heads\/main'/);
  assert.match(text, /environment: windows-signing/);
  assert.match(text, /runs-on: \[self-hosted, Windows, X64, fbforge-signing\]/);
  assert.match(text, /needs: verify-source/);
  assert.match(text, /needs: sign/);
  assert.match(text, /build-signed-windows\.ps1 -CI -Thumbprint/);
  assert.doesNotMatch(
    text,
    /pull_request_target:|workflow_run:|contents: write|gh release|softprops|pfx/i,
  );
});
