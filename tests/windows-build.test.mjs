import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, copyFile, rm, readdir, readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const pwsh = process.env.INSPECT_PWSH_EXECUTABLE || 'pwsh';
const available =
  process.platform === 'linux' &&
  spawnSync(pwsh, ['-NoProfile', '-Command', 'exit 0']).status === 0;

async function buildFixture(failCopy) {
  const root = await mkdtemp(join(tmpdir(), 'inspect-windows-build-'));
  for (const name of [
    'frontend',
    'desktop',
    'scripts',
    'docs',
    'assets',
    'licenses',
    'tests',
    'engine/target',
    'jdk/bin',
    'jdk/legal/java.base',
  ])
    await mkdir(join(root, name), { recursive: true });
  for (const name of [
    'package.json',
    'package-lock.json',
    'tsconfig.json',
    'LICENSE',
    'THIRD_PARTY_NOTICES.md',
    'engine/target/inspect-engine-0.1.0.jar',
  ])
    await writeFile(join(root, name), '{}');
  await copyFile('scripts/build-windows.ps1', join(root, 'scripts/build-windows.ps1'));
  await writeFile(join(root, 'jdk/legal/java.base/LICENSE'), 'GNU General Public License');
  await writeFile(join(root, 'jdk/legal/java.base/ADDITIONAL_LICENSE_INFO'), 'Classpath Exception');
  // This Linux-hosted harness tests PowerShell control flow, not Windows compilation.
  await writeFile(
    join(root, 'jdk/bin/java.exe'),
    '#!/bin/sh\nif [ "$1" = "--version" ]; then echo "openjdk 25.0.5"; else cat >/dev/null; echo \'{"ok":true}\'; fi\n',
    { mode: 0o755 },
  );
  await writeFile(
    join(root, 'harness.ps1'),
    String.raw`
param([string]$Fixture)
$ErrorActionPreference = 'Stop'
function global:robocopy {
  param([string]$Source, [string]$Destination)
  if ($env:INSPECT_TEST_COPY_FAILURE -eq '1') { $global:LASTEXITCODE = 8; return }
  $null = New-Item -ItemType Directory -Path $Destination -Force
  Get-ChildItem -LiteralPath $Source -Force | Copy-Item -Destination $Destination -Recurse -Force
  $global:LASTEXITCODE = 1 # Windows robocopy: files copied successfully.
}
function global:npm.cmd { $global:LASTEXITCODE = 0 }
function global:cargo.exe { $global:LASTEXITCODE = 0 }
function global:node.exe {
  $null = New-Item -ItemType Directory -Path target/release/engine, target/release/bundle/nsis -Force
  Set-Content target/release/fbforge-inspect.exe 'synthetic executable'
  Set-Content target/release/engine/inspect-engine.jar 'synthetic engine'
  Set-Content target/release/bundle/nsis/inspect-setup.exe 'synthetic installer'
  $global:LASTEXITCODE = 0
}
& (Join-Path $Fixture 'scripts/build-windows.ps1') -InspectJavaHome (Join-Path $Fixture 'jdk') -InspectWorkDirectory (Join-Path $Fixture 'work')
# GitHub's PowerShell wrapper propagates the final native exit code.
exit $LASTEXITCODE
`,
  );
  // The staging npm invocation normally creates the runtime. Provide just its probe executable.
  await mkdir(join(root, 'desktop/resources/engine/runtime/bin'), { recursive: true });
  await copyFile(
    join(root, 'jdk/bin/java.exe'),
    join(root, 'desktop/resources/engine/runtime/bin/java.exe'),
  );
  // robocopy deliberately excludes resources, so this stub supplies staging as npm would.
  const harness = join(root, 'harness.ps1');
  await writeFile(
    harness,
    (await readFile(harness, 'utf8')).replace(
      'function global:npm.cmd { $global:LASTEXITCODE = 0 }',
      String.raw`function global:npm.cmd {
      if ($args -contains 'stage:engine') {
        $null = New-Item -ItemType Directory -Path desktop/resources/engine/runtime/bin -Force
        Copy-Item (Join-Path $env:INSPECT_TEST_FIXTURE 'jdk/bin/java.exe') desktop/resources/engine/runtime/bin/java.exe -Force
      }
      $global:LASTEXITCODE = 0
    }`,
    ),
  );
  const result = spawnSync(pwsh, ['-NoProfile', '-NonInteractive', '-File', harness, root], {
    encoding: 'utf8',
    timeout: 30000,
    env: {
      ...process.env,
      INSPECT_TEST_FIXTURE: root,
      INSPECT_TEST_COPY_FAILURE: failCopy ? '1' : '0',
    },
  });
  return { root, result };
}

test(
  'successful robocopy codes do not fail the completed Windows build in CI',
  { skip: !available && 'Linux PowerShell harness unavailable' },
  async () => {
    const { root, result } = await buildFixture(false);
    try {
      assert.equal(result.status, 0, result.stdout + result.stderr);
      const files = await readdir(join(root, '.local/artifacts/windows-x64'));
      assert.ok(files.includes('inspect-setup.exe'));
      assert.ok(files.includes('SHA256SUMS.txt'));
      const artifacts = join(root, '.local/artifacts/windows-x64');
      const manifest = await readFile(join(artifacts, 'SHA256SUMS.txt'), 'utf8');
      assert.match(manifest, /engine\/inspect-engine\.jar/);
      for (const line of manifest.trim().split(/\r?\n/)) {
        const [hash, name] = line.split('  ');
        assert.equal(
          createHash('sha256')
            .update(await readFile(join(artifacts, name)))
            .digest('hex'),
          hash,
        );
      }
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  },
);

test(
  'a failed robocopy still aborts Windows packaging',
  { skip: !available && 'Linux PowerShell harness unavailable' },
  async () => {
    const { root, result } = await buildFixture(true);
    try {
      assert.notEqual(result.status, 0);
      assert.match(result.stdout + result.stderr, /Kopieren von frontend fehlgeschlagen/);
      await assert.rejects(readdir(join(root, '.local/artifacts/windows-x64')));
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  },
);

test(
  'hosted WebView2 test overrides reject a different repository before registry access',
  { skip: !available && 'Linux PowerShell harness unavailable' },
  () => {
    const result = spawnSync(
      pwsh,
      [
        '-NoProfile',
        '-NonInteractive',
        '-File',
        'scripts/ci/webview-test-policy.ps1',
        '-Mode',
        'enable',
        '-Directory',
        tmpdir(),
        '-Port',
        '9222',
      ],
      {
        encoding: 'utf8',
        timeout: 10000,
        env: {
          ...process.env,
          GITHUB_REPOSITORY: 'unrelated/repository',
          GITHUB_EVENT_NAME: 'push',
          RUNNER_ENVIRONMENT: 'github-hosted',
        },
      },
    );
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /ui_policy_context_denied/);
  },
);
