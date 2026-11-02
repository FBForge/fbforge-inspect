import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createHash } from 'node:crypto';
import { runtimeMetadata, runtimeNotice, verifyRuntime } from '../scripts/runtime-metadata.mjs';

const settings = {
  'java.version': '25.0.1',
  'java.runtime.version': '25.0.1+8-LTS',
  'java.vendor': 'Eclipse Adoptium',
  'os.name': 'Windows 11',
  'os.arch': 'amd64',
};
const release = 'JAVA_VERSION="25.0.1"\n';
const jdkRelease =
  release +
  'JAVA_RUNTIME_VERSION="25.0.1+8-LTS"\nIMPLEMENTOR_VERSION="Temurin-25.0.1+8"\nSOURCE=".:git:61d6a2c1b35e"\nSOURCE_REPO="https://github.com/adoptium/jdk25u.git"\n';
const input = {
  jdkRelease,
  selected: settings,
  bundled: settings,
  runtimeRelease: release,
  javaBytes: Buffer.from('synthetic Java executable'),
};

test('runtime attribution follows the selected source revision and actual bundled version', () => {
  const metadata = runtimeMetadata(input);
  assert.equal(metadata.runtimeVersion, '25.0.1+8-LTS');
  assert.equal(metadata.source.revision, '61d6a2c1b35e');
  assert.equal(
    metadata.source.url,
    'https://github.com/adoptium/jdk25u/archive/61d6a2c1b35e.tar.gz',
  );
  assert.ok(runtimeNotice(metadata).includes(metadata.runtimeVersion));
  assert.ok(!runtimeNotice(metadata).includes('25.0.5'));
});

test('stale runtime patch, build, vendor and platform are rejected before attribution', () => {
  for (const [field, value] of [
    ['java.version', '25.0.0'],
    ['java.runtime.version', '25.0.1+7-LTS'],
    ['java.vendor', 'Other vendor'],
    ['os.arch', 'aarch64'],
  ])
    assert.throws(
      () => runtimeMetadata({ ...input, bundled: { ...settings, [field]: value } }),
      /Laufzeit/,
    );
  assert.throws(
    () => runtimeMetadata({ ...input, runtimeRelease: 'JAVA_VERSION="25.0.0"\n' }),
    /Versionen/,
  );
  assert.throws(
    () =>
      runtimeMetadata({ ...input, jdkRelease: jdkRelease.replace('25.0.1+8-LTS', '25.0.1+7-LTS') }),
    /widersprechen/,
  );
});

test('missing source identity cannot silently select an unrelated fixed release', () => {
  assert.throws(() => runtimeMetadata({ ...input, jdkRelease: release }), /Quellenreferenz fehlt/);
  const metadata = runtimeMetadata({
    ...input,
    jdkRelease: release,
    sourceUrl: 'https://example.org/exact-build-sources.tar.gz',
  });
  assert.equal(metadata.source.origin, 'explicit build input');
  assert.equal(metadata.source.url, 'https://example.org/exact-build-sources.tar.gz');
  for (const sourceUrl of [
    'http://example.org/src.zip',
    'https://user:secret@example.org/src.zip',
    'https://example.org/src.zip?token=secret',
  ])
    assert.throws(() => runtimeMetadata({ ...input, sourceUrl }), /Quellenreferenz/);
});

test('packaged runtime verification detects changed binaries, identity and notices', async () => {
  const root = await mkdtemp(join(tmpdir(), 'inspect-runtime-metadata-'));
  try {
    const bin = join(root, 'runtime/bin', process.platform === 'win32' ? 'java.exe' : 'java');
    await mkdir(join(root, 'runtime/bin'), { recursive: true });
    await writeFile(bin, input.javaBytes);
    await writeFile(join(root, 'runtime/release'), release);
    const metadata = runtimeMetadata(input),
      notice = '# Distribution notices\n' + runtimeNotice(metadata);
    metadata.noticeSha256 = createHash('sha256').update(notice).digest('hex');
    await writeFile(join(root, 'runtime-provenance.json'), JSON.stringify(metadata));
    await writeFile(join(root, 'THIRD_PARTY_NOTICES.md'), notice);
    assert.equal(
      (await verifyRuntime(root, () => settings)).runtimeVersion,
      settings['java.runtime.version'],
    );
    assert.equal(
      (await verifyRuntime(root, () => ({ ...settings, 'os.name': 'Windows Server 2025' }))).os,
      'windows',
    );
    await assert.rejects(
      verifyRuntime(root, () => ({ ...settings, 'java.runtime.version': '25.0.1+9' })),
      /andere Java/,
    );
    await writeFile(bin, 'changed binary');
    await assert.rejects(
      verifyRuntime(root, () => settings),
      /Prüfsummen/,
    );
    await writeFile(bin, input.javaBytes);
    await writeFile(
      join(root, 'THIRD_PARTY_NOTICES.md'),
      notice.replace('25.0.1+8-LTS', '25.0.5+8'),
    );
    await assert.rejects(
      verifyRuntime(root, () => settings),
      /Laufzeit-Hinweis/,
    );
    assert.equal(await readFile(join(root, 'runtime/release'), 'utf8'), release);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
