import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, writeFile, rm, readdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { powershellRuntime } from './powershell.mjs';

const { executable: shell, environment } = powershellRuntime();
const available =
  spawnSync(shell, ['-NoProfile', '-Command', 'exit 0'], { env: environment }).status === 0;
const digest = (text) => createHash('sha256').update(text).digest('hex');
for (const [scenario, expected] of [
  ['changed file', 'signed_checksum_mismatch'],
  ['extra file', 'signed_checksum_inventory_mismatch'],
  ['parent path', 'signed_checksum_path_invalid'],
  ['duplicate path', 'signed_checksum_path_invalid'],
  ['wrong commit', 'signed_manifest_context_mismatch'],
  ['wrong validation run', 'signed_manifest_context_mismatch'],
  ['wrong signer identity', 'signed_manifest_context_mismatch'],
  ['wrong installer bytes', 'signed_installer_integrity_failed'],
]) {
  test(
    `signed package rejects ${scenario} before platform signature lookup`,
    { skip: !available && 'PowerShell unavailable' },
    async () => {
      const directory = await mkdtemp(join(tmpdir(), 'inspect-package-'));
      try {
        const manifest = {
          application: 'FBForge Inspect',
          platform: 'windows-x64',
          version: '0.1.0',
          repository: 'FBForge/fbforge-inspect',
          commit: 'a'.repeat(40),
          runId: '123',
          validationRunId: '456',
          signing: 'authenticode',
          certificateThumbprint: 'b'.repeat(40),
          installer: 'fictional-setup.exe',
          published: false,
          bytes: Buffer.byteLength('fictional setup'),
          sha256: digest('fictional setup'),
        };
        if (scenario === 'wrong commit') manifest.commit = 'c'.repeat(40);
        if (scenario === 'wrong validation run') manifest.validationRunId = '789';
        if (scenario === 'wrong signer identity') manifest.certificateThumbprint = 'c'.repeat(40);
        if (scenario === 'wrong installer bytes') manifest.bytes++;
        const contents = {
          'build-info.json': JSON.stringify(manifest),
          'fictional-setup.exe': 'fictional setup',
          'fbforge-inspect.exe': 'fictional application',
        };
        for (const [name, text] of Object.entries(contents))
          await writeFile(join(directory, name), text);
        const hashes = Object.entries(contents).map(([name, text]) => `${digest(text)}  ${name}`);
        if (scenario === 'changed file')
          await writeFile(join(directory, 'fbforge-inspect.exe'), 'changed application');
        if (scenario === 'extra file') await writeFile(join(directory, 'unlisted.exe'), 'extra');
        if (scenario === 'parent path') hashes.unshift(`${digest('outside')}  ../outside.exe`);
        if (scenario === 'duplicate path') hashes.push(hashes[0]);
        await writeFile(join(directory, 'SHA256SUMS.txt'), hashes.join('\n') + '\n');
        const result = spawnSync(
          shell,
          [
            '-NoProfile',
            '-NonInteractive',
            '-ExecutionPolicy',
            'Bypass',
            '-File',
            'scripts/ci/check-signed-package.ps1',
            '-Directory',
            directory,
            '-Thumbprint',
            'b'.repeat(40),
          ],
          {
            encoding: 'utf8',
            timeout: 15000,
            env: {
              ...environment,
              GITHUB_SHA: 'a'.repeat(40),
              GITHUB_RUN_ID: '123',
              VALIDATION_RUN: '456',
            },
          },
        );
        assert.notEqual(result.status, 0);
        assert.match(result.stderr, new RegExp(expected), result.stderr);
        assert.ok(!(await readdir(directory)).includes('signature-info.json'));
      } finally {
        await rm(directory, { recursive: true, force: true });
      }
    },
  );
}
