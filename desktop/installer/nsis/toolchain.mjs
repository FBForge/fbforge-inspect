// Build tools only. Runtime download and execution never use this module.
import { createHash } from 'node:crypto';
import { readFile, writeFile, mkdir, mkdtemp, rm, rename, lstat, readdir } from 'node:fs/promises';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath, pathToFileURL } from 'node:url';
import path from 'node:path';

const execute = promisify(execFile);
const assets = fileURLToPath(new URL('.', import.meta.url));
const names = ['bzip2', 'bzip2_solid', 'lzma', 'lzma_solid', 'zlib', 'zlib_solid'].map(name => `${name}-x86-unicode`);
const archiveUrl = 'https://github.com/tauri-apps/binary-releases/releases/download/nsis-3.13/nsis-3.13.zip';
const pluginUrl = 'https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/nsis_tauri_utils.dll';
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
function fail() { const error = new Error('The pinned installer toolchain is unavailable.'); error.code = 'toolchainMismatch'; throw error; }
async function pins() {
  const value = JSON.parse(await readFile(path.join(assets, 'manifest.json'), 'utf8'));
  if (value.schemaVersion !== 1 || value.nsisVersion !== '3.13'
    || Object.keys(value.stubs).sort().join(',') !== names.toSorted().join(',')
    || Object.keys(value.compilerFiles).sort().join(',') !== 'Bin/makensis.exe,NSIS.exe,makensis.exe') fail();
  return value;
}
async function checkedFile(file, pin) {
  const info = await lstat(file);
  if (!info.isFile() || info.size !== pin.size || info.size > 1024 * 1024) fail();
  const bytes = await readFile(file);
  if (bytes.length !== pin.size || digest(bytes) !== pin.sha256) fail();
  return bytes;
}

async function toolsetDigest(directory) {
  const files = []; let total = 0; let count = 0;
  async function visit(relative, depth) {
    if (depth > 16) fail();
    for (const name of await readdir(path.join(directory, relative))) {
      if (++count > 4096) fail();
      const child = relative ? `${relative}/${name}` : name;
      const file = path.join(directory, child);
      const info = await lstat(file);
      if (info.isDirectory()) { await visit(child, depth + 1); continue; }
      if (!info.isFile() || info.size > 16 * 1024 * 1024 || (total += info.size) > 128 * 1024 * 1024) fail();
      const bytes = await readFile(file);
      if (bytes.length !== info.size) fail();
      files.push({ name: Buffer.from(child), size: bytes.length, hash: createHash('sha256').update(bytes).digest() });
    }
  }
  await visit('', 0);
  files.sort((a, b) => Buffer.compare(a.name, b.name));
  const digest = createHash('sha256').update('FBForge NSIS toolset v1\0');
  for (const file of files) {
    const nameSize = Buffer.alloc(4); nameSize.writeUInt32LE(file.name.length);
    const size = Buffer.alloc(8); size.writeBigUInt64LE(BigInt(file.size));
    digest.update(nameSize).update(file.name).update(size).update(file.hash);
  }
  return digest.digest('hex');
}

export async function verifyNsisTools(directory) {
  try {
    const pinned = await pins();
    for (const name of names) await checkedFile(path.join(directory, 'Stubs', name), pinned.stubs[name]);
    for (const [name, pin] of Object.entries(pinned.compilerFiles)) await checkedFile(path.join(directory, name), pin);
    const plugin = path.join(directory, 'Plugins', 'x86-unicode', 'additional', 'nsis_tauri_utils.dll');
    const info = await lstat(plugin);
    if (!info.isFile() || info.size > 1024 * 1024 || digest(await readFile(plugin)) !== pinned.tauriPluginSha256) fail();
    if (await toolsetDigest(directory) !== pinned.toolsetSha256) fail();
  } catch { fail(); }
}

async function publicBytes(file, url, expected, maximum) {
  let bytes;
  if (file) {
    const info = await lstat(file);
    if (!info.isFile() || info.size > maximum) fail();
    bytes = await readFile(file);
  } else {
    const signal = AbortSignal.timeout(60_000);
    let current = new URL(url);
    const hosts = new Set(['github.com', 'objects.githubusercontent.com', 'release-assets.githubusercontent.com']);
    for (let count = 0; count < 8; count++) {
      if (current.protocol !== 'https:' || current.port || current.username || current.password || !hosts.has(current.hostname)) fail();
      const response = await fetch(current, { redirect: 'manual', signal });
      if ([301, 302, 303, 307, 308].includes(response.status)) {
        const location = response.headers.get('location');
        await response.body?.cancel();
        if (!location) fail();
        current = new URL(location, current);
        continue;
      }
      if (!response.ok || !response.body) fail();
      const chunks = []; let size = 0;
      for await (const chunk of response.body) {
        size += chunk.length;
        if (size > maximum) fail();
        chunks.push(chunk);
      }
      bytes = Buffer.concat(chunks, size);
      break;
    }
  }
  if (!bytes || bytes.length > maximum || digest(bytes) !== expected) fail();
  return bytes;
}

async function extractPinnedArchive(archive, destination) {
  // Extraction follows the exact public SHA256 check, never an untrusted ZIP.
  if (process.platform === 'win32') {
    const quote = value => `'${value.replaceAll("'", "''")}'`;
    // Windows PowerShell serializes progress as CLIXML on bounded stderr.
    const script = `$ErrorActionPreference='Stop'; $ProgressPreference='SilentlyContinue'; Expand-Archive -LiteralPath ${quote(archive)} -DestinationPath ${quote(destination)}`;
    await execute('powershell.exe', ['-NoProfile', '-NonInteractive', '-EncodedCommand', Buffer.from(script, 'utf16le').toString('base64')], { timeout: 60_000, maxBuffer: 65536, windowsHide: true });
  } else {
    await execute('python3', ['-c', 'import zipfile,sys; zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])', archive, destination], { timeout: 60_000, maxBuffer: 65536 });
  }
}

export async function prepareNsisTools({ target, archive, plugin }) {
  const cache = path.join(path.resolve(target), '.tauri');
  const destination = path.join(cache, 'NSIS');
  try { await verifyNsisTools(destination); return destination; } catch { /* A fresh private replacement follows. */ }
  const pinned = await pins();
  const archiveBytes = await publicBytes(archive, archiveUrl, pinned.archiveSha256, 32 * 1024 * 1024);
  const pluginBytes = await publicBytes(plugin, pluginUrl, pinned.tauriPluginSha256, 1024 * 1024);
  await mkdir(cache, { recursive: true });
  const scratch = await mkdtemp(path.join(cache, '.fbforge-nsis-'));
  try {
    const archivePath = path.join(scratch, 'nsis.zip');
    await writeFile(archivePath, archiveBytes, { flag: 'wx', mode: 0o600 });
    await extractPinnedArchive(archivePath, path.join(scratch, 'extracted'));
    const prepared = path.join(scratch, 'extracted', 'nsis-3.13');
    for (const name of names) {
      const bytes = await checkedFile(path.join(assets, 'Stubs', name), pinned.stubs[name]);
      await writeFile(path.join(prepared, 'Stubs', name), bytes);
    }
    const plugins = path.join(prepared, 'Plugins', 'x86-unicode', 'additional');
    await mkdir(plugins, { recursive: true });
    await writeFile(path.join(plugins, 'nsis_tauri_utils.dll'), pluginBytes, { flag: 'wx' });
    await verifyNsisTools(prepared);
    // Preserve any previous toolset rather than deleting another build's files.
    try { await rename(destination, path.join(cache, `${path.basename(scratch)}-previous`)); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
    await rename(prepared, destination);
    return destination;
  } finally { await rm(scratch, { recursive: true, force: true }); }
}

async function main() {
  if (process.argv.length !== 3 || process.argv[2] !== 'prepare' || process.versions.node.split('.')[0] !== '24') fail();
  if (process.env.TAURI_ENV_PLATFORM && process.env.TAURI_ENV_PLATFORM !== 'windows') return;
  const result = await execute('cargo', ['metadata', '--format-version', '1', '--no-deps', '--locked', '--offline', '--manifest-path', 'desktop/Cargo.toml'], { timeout: 60_000, maxBuffer: 4 * 1024 * 1024 });
  const target = JSON.parse(result.stdout).target_directory;
  if (typeof target !== 'string' || !path.isAbsolute(target)) fail();
  await prepareNsisTools({ target });
  console.log('Pinned project-local NSIS tools prepared.');
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch(() => { console.error('Pinned installer toolchain preparation failed.'); process.exitCode = 1; });
}
