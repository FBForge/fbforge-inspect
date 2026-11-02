import { readFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
const platform = (name) =>
  name.startsWith('Windows') ? 'windows' : name === 'Mac OS X' ? 'macos' : name.toLowerCase();
const architecture = (name) =>
  ['amd64', 'x86_64'].includes(name) ? 'x64' : ['aarch64', 'arm64'].includes(name) ? 'arm64' : name;
export function releaseProperties(text) {
  return Object.fromEntries(
    [...String(text).matchAll(/^([A-Z_]+)="([^"\r\n]*)"\s*$/gm)].map((m) => [m[1], m[2]]),
  );
}

export function javaSettings(executable) {
  const result = spawnSync(executable, ['-XshowSettings:properties', '-version'], {
    encoding: 'utf8',
    timeout: 15000,
    maxBuffer: 1024 * 1024,
  });
  if (result.error || result.status !== 0) throw new Error('Java-Laufzeitdaten nicht lesbar.');
  const values = Object.fromEntries(
    [...(result.stderr + result.stdout).matchAll(/^\s*([a-z.]+) = ([^\r\n]+)$/gm)].map((m) => [
      m[1],
      m[2].trim(),
    ]),
  );
  for (const key of [
    'java.home',
    'java.version',
    'java.runtime.version',
    'java.vendor',
    'os.name',
    'os.arch',
  ])
    if (!values[key]) throw new Error('Java-Laufzeitdaten unvollständig.');
  return values;
}

function publicSourceUrl(value) {
  const url = new URL(value);
  if (url.protocol !== 'https:' || url.username || url.password || url.search || url.hash)
    throw new Error('Öffentliche HTTPS-Quellenreferenz ohne Zugangsdaten erwartet.');
  return url.toString();
}

function gitSource(repository, revision) {
  if (!repository || !revision) return null;
  const url = new URL(publicSourceUrl(repository));
  const path = url.pathname.replace(/\.git$/, '').replace(/\/$/, '');
  if (
    url.hostname !== 'github.com' ||
    !/^\/[\w.-]+\/[\w.-]+$/.test(path) ||
    !/^[a-f0-9]{12,40}$/.test(revision)
  )
    throw new Error(
      'JDK-Quellenreferenz nicht eindeutig. INSPECT_JAVA_SOURCE_URL ausdrücklich setzen.',
    );
  return {
    repository: `${url.origin}${path}`,
    revision,
    url: `${url.origin}${path}/archive/${revision}.tar.gz`,
  };
}

export function runtimeMetadata({
  jdkRelease,
  selected,
  bundled,
  runtimeRelease,
  javaBytes,
  sourceUrl,
}) {
  const jdk = releaseProperties(jdkRelease),
    runtime = releaseProperties(runtimeRelease);
  if (
    !/^25(?:\.|$)/.test(selected['java.version']) ||
    jdk.JAVA_VERSION !== selected['java.version'] ||
    runtime.JAVA_VERSION !== selected['java.version']
  )
    throw new Error(
      'JDK und gebündelte Laufzeit haben unterschiedliche Java-Versionen. Frisches Buildverzeichnis verwenden.',
    );
  for (const key of ['java.version', 'java.runtime.version', 'java.vendor', 'os.name', 'os.arch'])
    if (selected[key] !== bundled[key])
      throw new Error(
        'Gebündelte Laufzeit passt nicht zum gewählten JDK. Frisches Buildverzeichnis verwenden.',
      );
  if (jdk.JAVA_RUNTIME_VERSION && jdk.JAVA_RUNTIME_VERSION !== selected['java.runtime.version'])
    throw new Error('JDK-release und laufendes Java widersprechen sich.');
  const revision = jdk.SOURCE?.match(/(?:^|\s)\.:git:([a-f0-9]{12,40})(?:\s|$)/)?.[1];
  const source = sourceUrl
    ? { url: publicSourceUrl(sourceUrl), origin: 'explicit build input' }
    : gitSource(jdk.SOURCE_REPO, revision);
  if (!source)
    throw new Error(
      'Passende JDK-Quellenreferenz fehlt. INSPECT_JAVA_SOURCE_URL für die gewählte Distribution setzen.',
    );
  const buildRevision = jdk.BUILD_SOURCE?.match(/^git:([a-f0-9]{12,40})$/)?.[1];
  return {
    schemaVersion: 1,
    javaVersion: bundled['java.version'],
    runtimeVersion: bundled['java.runtime.version'],
    vendor: bundled['java.vendor'],
    implementorVersion: jdk.IMPLEMENTOR_VERSION || null,
    os: platform(bundled['os.name']),
    arch: architecture(bundled['os.arch']),
    source,
    buildSource: gitSource(jdk.BUILD_SOURCE_REPO, buildRevision),
    jdkReleaseSha256: sha256(jdkRelease),
    runtimeReleaseSha256: sha256(runtimeRelease),
    javaExecutableSha256: sha256(javaBytes),
  };
}

export function runtimeNotice(metadata) {
  const code = (value) => String(value).replace(/[`\r\n]/g, ' ');
  return `\n## Bundled OpenJDK runtime\n\nThis package includes ${code(metadata.vendor)} ${code(metadata.runtimeVersion)} for ${code(metadata.os)} / ${code(metadata.arch)} (Java ${code(metadata.javaVersion)}).\n\nCorresponding OpenJDK sources: ${metadata.source.url}\n${metadata.buildSource ? `\nDistribution build sources: ${metadata.buildSource.url}\n` : ''}\nThe selected JDK source reference and the bundled runtime hashes are recorded in \`engine/runtime-provenance.json\`. Retain the runtime's \`legal/\` directory and provide the corresponding sources when distributing this runtime.\n`;
}

export async function verifyRuntime(directory, inspect = javaSettings) {
  const metadata = JSON.parse(await readFile(join(directory, 'runtime-provenance.json'), 'utf8'));
  const runtime = join(directory, 'runtime');
  const executable = join(runtime, 'bin', process.platform === 'win32' ? 'java.exe' : 'java');
  const release = await readFile(join(runtime, 'release'));
  const settings = inspect(executable);
  if (
    metadata.schemaVersion !== 1 ||
    metadata.javaExecutableSha256 !== sha256(await readFile(executable)) ||
    metadata.runtimeReleaseSha256 !== sha256(release)
  )
    throw new Error('Laufzeit-Prüfsummen passen nicht zur Paketbeilage.');
  for (const [field, property] of [
    ['javaVersion', 'java.version'],
    ['runtimeVersion', 'java.runtime.version'],
    ['vendor', 'java.vendor'],
  ])
    if (metadata[field] !== settings[property])
      throw new Error('Paketbeilage beschreibt eine andere Java-Laufzeit.');
  if (
    metadata.os !== platform(settings['os.name']) ||
    metadata.arch !== architecture(settings['os.arch'])
  )
    throw new Error('Paketbeilage beschreibt eine andere Zielplattform.');
  if (releaseProperties(release).JAVA_VERSION !== metadata.javaVersion)
    throw new Error('Laufzeit-release und Paketbeilage widersprechen sich.');
  publicSourceUrl(metadata.source.url);
  const notice = await readFile(join(directory, 'THIRD_PARTY_NOTICES.md'), 'utf8');
  if (sha256(notice) !== metadata.noticeSha256 || !notice.endsWith(runtimeNotice(metadata)))
    throw new Error('Laufzeit-Hinweis passt nicht zur Paketbeilage.');
  return metadata;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv[2] !== '--verify' || !process.argv[3])
    throw new Error('Usage: runtime-metadata.mjs --verify <engine-directory>');
  const metadata = await verifyRuntime(resolve(process.argv[3]));
  console.log(`Runtime provenance verified: ${metadata.vendor} ${metadata.runtimeVersion}`);
}
