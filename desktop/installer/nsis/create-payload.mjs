// Build-time byte inventory. Its provenance comes from the signed distribution,
// never from a checksum alone. Inputs are the exact rendered NSIS File map.
import { createHash } from 'node:crypto';
import { open, readFile, lstat } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { verifyNsisTools } from './toolchain.mjs';

const MAX_INPUT = 16 * 1024 * 1024;
const MAX_MANIFEST = 4 * 1024 * 1024;
const MAX_BYTES = 32 * 1024 * 1024 * 1024;
const reserved = new Set(['uninstall.exe', '.fbforge-install.pending', '.fbforge-installation.v1.json']);
function fail(code = 'invalidMap') { const error = new Error(code); error.code = code; throw error; }
function record(value) { return typeof value === 'object' && value !== null && !Array.isArray(value); }
function identifier(value, max = 128) {
  return typeof value === 'string' && value.length <= max && /^[a-z0-9](?:[a-z0-9._-]*[a-z0-9])?$/i.test(value);
}
function relativeFile(value) {
  if (typeof value !== 'string' || /^[\\/]/.test(value)) fail();
  const normalized = value.replaceAll('/', '\\').replace(/\\+/g, '\\');
  if (normalized.length > 512 || normalized.split('\\').length > 32 || reserved.has(normalized.split('\\')[0].toLowerCase())) fail();
  for (const component of normalized.split('\\')) {
    const device = component.split('.')[0];
    if (!component || component.length > 255 || component.trim() !== component || component.endsWith('.')
      || /[\x00-\x1f\x7f<>:"|?*]/.test(component)
      || /^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])$/i.test(device)) fail();
  }
  return normalized;
}
function parseMap(input) {
  if (typeof input !== 'string' || Buffer.byteLength(input) > MAX_INPUT) fail();
  const lines = input.replace(/^\uFEFF/, '').replaceAll('\r\n', '\n').split('\n');
  if (lines.at(-1) === '') lines.pop();
  let header;
  try { header = JSON.parse(lines.shift()); } catch { fail(); }
  if (!record(header) || Object.keys(header).sort().join(',') !== 'appId,optionalPrefixes,schemaVersion,toolId,version'
    || header.schemaVersion !== 1 || !identifier(header.toolId) || !identifier(header.appId)
    || typeof header.version !== 'string' || header.version.length > 128
    || !/^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(header.version)
    || !Array.isArray(header.optionalPrefixes) || header.optionalPrefixes.length > 8
    || !lines.length || lines.length % 2 || lines.length / 2 > 8192) fail();
  const prefixes = header.optionalPrefixes.map(item => {
    if (!record(item) || Object.keys(item).sort().join(',') !== 'component,prefix'
      || !identifier(item.component, 64) || typeof item.prefix !== 'string' || !/[\\/]$/.test(item.prefix)) fail();
    return { prefix: `${relativeFile(item.prefix.slice(0, -1)).toLowerCase()}\\`, component: item.component };
  });
  for (let i = 0; i < prefixes.length; i++) {
    if (prefixes.some((other, j) => i !== j && prefixes[i].prefix.startsWith(other.prefix))) fail();
  }
  const seen = new Set(); const entries = []; let pathUnits = 0;
  for (let i = 0; i < lines.length; i += 2) {
    const source = lines[i]; const target = relativeFile(lines[i + 1]); const key = target.toLowerCase();
    if (!path.isAbsolute(source) || /[\x00-\x1f\x7f]/.test(source) || source.length > 32767 || seen.has(key)) fail();
    seen.add(key); pathUnits += target.length;
    if (pathUnits > 4 * 1024 * 1024) fail();
    entries.push({ source, path: target, component: prefixes.find(item => key.startsWith(item.prefix))?.component ?? null });
  }
  for (const entry of entries) {
    const parts = entry.path.toLowerCase().split('\\');
    for (let i = 1; i < parts.length; i++) if (seen.has(parts.slice(0, i).join('\\'))) fail();
  }
  return { header, entries };
}

async function hashSource(source, remaining) {
  let file;
  try {
    // Refuse special files before opening: a FIFO must not stall a build.
    const before = await lstat(source, { bigint: true });
    if (!before.isFile()) fail('unreadableFile');
    if (before.size > BigInt(remaining)) fail('tooLarge');
    file = await open(source, 'r');
    const opened = await file.stat({ bigint: true });
    if (opened.dev !== before.dev || opened.ino !== before.ino || opened.size !== before.size) fail('fileChanged');
    const digest = createHash('sha256'); let size = 0;
    for await (const chunk of file.createReadStream({ highWaterMark: 65536, autoClose: false })) {
      size += chunk.length;
      if (size > remaining || BigInt(size) > before.size) fail('fileChanged');
      digest.update(chunk);
    }
    const after = await file.stat({ bigint: true });
    if (BigInt(size) !== before.size || after.size !== before.size || after.mtimeNs !== before.mtimeNs
      || after.ctimeNs !== before.ctimeNs) fail('fileChanged');
    return { size, sha256: digest.digest('hex') };
  } catch (error) {
    if (['unreadableFile', 'tooLarge', 'fileChanged'].includes(error?.code)) throw error;
    fail('unreadableFile');
  } finally { await file?.close(); }
}

export async function buildPayloadManifest(input) {
  const { header, entries } = parseMap(input);
  entries.sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
  const files = []; let total = 0;
  for (const entry of entries) {
    const bytes = await hashSource(entry.source, MAX_BYTES - total); total += bytes.size;
    files.push({ path: entry.path, ...bytes, component: entry.component });
  }
  const result = { schemaVersion: 1, toolId: header.toolId, appId: header.appId, version: header.version, files };
  if (Buffer.byteLength(JSON.stringify(result)) + 1 > MAX_MANIFEST) fail('tooLarge');
  return result;
}

async function writeCompilerOutput(output, text) {
  // !tempfile creates an empty private output. Never truncate an unrelated file.
  const before = await lstat(output, { bigint: true });
  if (!before.isFile() || before.size !== 0n) fail('invalidOutput');
  const file = await open(output, 'r+');
  try {
    const opened = await file.stat({ bigint: true });
    if (opened.ino !== before.ino || opened.dev !== before.dev || opened.size !== 0n) fail('invalidOutput');
    await file.writeFile(text, 'utf8'); await file.sync();
  } finally { await file.close(); }
}

async function main() {
  if (process.argv.length !== 6 || process.versions.node.split('.')[0] !== '24') fail('unsupportedBuilder');
  const [input, output, definition, nsisDirectory] = process.argv.slice(2);
  await verifyNsisTools(nsisDirectory);
  const info = await lstat(input);
  if (!info.isFile() || info.size > MAX_INPUT) fail();
  const manifest = await buildPayloadManifest(await readFile(input, 'utf8'));
  const bytes = `${JSON.stringify(manifest)}\n`;
  const digest = createHash('sha256').update(bytes).digest('hex');
  await writeCompilerOutput(output, bytes);
  // Only this fixed-format digest becomes NSIS source. Map paths stay data.
  await writeCompilerOutput(definition, `!define FBFORGE_PAYLOAD_SHA256 "${digest}"\n`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch(error => {
    const codes = new Set(['invalidMap', 'unreadableFile', 'tooLarge', 'fileChanged', 'invalidOutput', 'unsupportedBuilder']);
    console.error(`FBForge payload: ${codes.has(error?.code) ? error.code : 'buildFailed'}`);
    process.exitCode = 1;
  });
}
