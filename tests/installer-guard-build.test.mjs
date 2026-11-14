import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const root = new URL('../', import.meta.url);
const digest = (bytes) => createHash('sha256').update(bytes).digest('hex');

test('the public guard snapshot preserves the pinned Suite source bytes', async () => {
  const base = new URL('desktop/installer/guard-source/', root);
  const manifest = JSON.parse(await readFile(new URL('source-manifest.json', base), 'utf8'));
  assert.equal(manifest.revision, '56456955edb52a00d7493283d95b44ead8176f6e');
  assert.equal(manifest.files.length, 117);
  for (const file of manifest.files) {
    const bytes = await readFile(new URL(file.path, base));
    assert.equal(bytes.length, file.size, file.path);
    assert.equal(digest(bytes), file.sha256, file.path);
  }
});

// Arbitrary bytes exercise preservation only; they are not a DLL or a signature fixture.
test('the signed-bundle hook only verifies the staged bytes and never rebuilds them', async () => {
  const fixture = await mkdtemp(join(tmpdir(), 'guard-staging-'));
  try {
    await mkdir(join(fixture, 'scripts'));
    await mkdir(join(fixture, 'desktop/installer/guard'), { recursive: true });
    const helper = join(fixture, 'scripts/installer-guard.mjs');
    await copyFile(new URL('scripts/installer-guard.mjs', root), helper);
    const file = join(fixture, 'desktop/installer/guard/fbforge_suite_nsis_guard.dll');
    const bytes = Buffer.from('byte-preservation fixture, not executable');
    await writeFile(file, bytes);
    const run = (...args) =>
      spawnSync(process.execPath, [helper, ...args], {
        encoding: 'utf8',
        env: { PATH: '', NODE_OPTIONS: '' },
        timeout: 10000,
      });
    assert.equal(run('verify', digest(bytes)).status, 0);
    assert.notEqual(run('verify', '0'.repeat(64)).status, 0);
    assert.notEqual(run('verify', 'not-a-digest').status, 0);
    assert.notEqual(run('verify', digest(bytes), 'extra').status, 0);
    assert.deepEqual(await readFile(file), bytes);
    await writeFile(file, Buffer.alloc(0));
    assert.notEqual(run('verify', digest(Buffer.alloc(0))).status, 0);
    await rm(file);
    assert.notEqual(run('verify', digest(bytes)).status, 0);
  } finally {
    await rm(fixture, { recursive: true, force: true });
  }
});

test('bundling builds the x86 guard while protected signing substitutes verification', async () => {
  const config = JSON.parse(await readFile(new URL('desktop/tauri.conf.json', root), 'utf8'));
  assert.equal(
    config.build.beforeBundleCommand,
    'node scripts/installer-guard.mjs && node desktop/installer/nsis/toolchain.mjs prepare',
  );
  const helper = await readFile(new URL('scripts/installer-guard.mjs', root), 'utf8');
  assert.match(helper, /i686-pc-windows-msvc/);
  assert.match(helper, /1\.93\.1/);
  assert.match(helper, /target-feature=\+crt-static/);
  assert.doesNotMatch(helper, /rustup[^\n]*install/);
  const signing = await readFile(new URL('scripts/build-windows.ps1', root), 'utf8');
  const buildAt = signing.indexOf('installer-guard.mjs');
  const signAt = signing.indexOf('Invoke-WindowsFileSigning', buildAt);
  const verifyAt = signing.indexOf('installer-guard.mjs verify', signAt);
  assert.ok(buildAt >= 0 && signAt > buildAt && verifyAt > signAt);
  assert.match(signing.slice(signAt, verifyAt), /Assert-WindowsSignature/);
});

test('unsigned CI asserts refusal while signed candidate keeps native accessibility acceptance', async () => {
  const ordinary = await readFile(new URL('.github/workflows/verify.yml', root), 'utf8');
  const protectedBuild = await readFile(
    new URL('.github/workflows/sign-windows.yml', root),
    'utf8',
  );
  assert.match(ordinary, /test-unsigned-installer\.ps1/);
  assert.doesNotMatch(ordinary, /windows-accessibility-smoke\.ps1/);
  assert.match(protectedBuild, /windows-accessibility-smoke\.ps1/);
  assert.match(
    ordinary,
    /rustup toolchain install 1\.93\.1 --profile minimal --target i686-pc-windows-msvc/,
  );
  assert.match(
    protectedBuild,
    /rustup toolchain install 1\.93\.1 --profile minimal --target i686-pc-windows-msvc/,
  );
});

test('unsigned refusal checks the clean native destination and both uninstall registry views', async () => {
  const script = await readFile(new URL('scripts/ci/test-unsigned-installer.ps1', root), 'utf8');
  assert.match(script, /\[Parameter\(Mandatory\)\]\[string\]\$ToolId/);
  assert.match(script, /guard-source\/catalog\/tools\.v1\.json/);
  assert.match(script, /\$catalog\.tools \| Where-Object \{ \$_\.id -ceq \$ToolId \}/);
  assert.match(script, /SpecialFolder\]::LocalApplicationData/);
  assert.match(script, /Join-Path \$localAppData \$tool\.productName/);
  assert.match(script, /foreach \(\$name in \$tool\.registryNames\)/);
  assert.match(script, /RegistryView\]::Registry64/);
  assert.match(script, /RegistryView\]::Registry32/);
  assert.match(
    script,
    /OpenSubKey\('Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\' \+ \$name, \$false\)/,
  );
  const before = script.indexOf("Assert-NativeFixtureAbsent 'unsigned_fixture_not_clean'");
  const launch = script.indexOf('Start-Process');
  const exited = script.indexOf('$process.ExitCode -ne 70');
  const after = script.indexOf(
    "Assert-NativeFixtureAbsent 'unsigned_installer_mutated_native_target'",
  );
  assert.ok(before >= 0 && launch > before && exited > launch && after > exited);
  const deletions = script
    .split('\n')
    .filter((line) => /Remove-Item|DeleteSubKey|DeleteValue/.test(line));
  assert.deepEqual(
    deletions.map((line) => line.trim()),
    ['Remove-Item -LiteralPath $root -Recurse -Force'],
  );
  assert.match(script, /Test-Path -LiteralPath \$target/);
  assert.match(script, /ReadAllText\(\$sentinel\)/);
  const callers = [await readFile(new URL('.github/workflows/verify.yml', root), 'utf8')];

  const invocations = callers.flatMap((source) =>
    source.split('\n').filter((line) => /test-unsigned-installer\.ps1.*-InstallerPath/.test(line)),
  );
  assert.equal(invocations.length, 1);
  for (const line of invocations) assert.match(line, /-ToolId fbforge-inspect(?:\s|$)/);
});
