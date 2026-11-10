const MAX_BYTES = 32 * 1024 * 1024;
const decoder = new TextDecoder('utf-8', { fatal: true });
export function crc32(bytes: Uint8Array) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
  }
  return (crc ^ 0xffffffff) >>> 0;
}
export async function readZip(bytes: Uint8Array): Promise<Map<string, string>> {
  if (bytes.length > 8 * 1024 * 1024 || bytes.length < 22)
    throw new Error('XLSX-Datei fehlt oder überschreitet 8 MiB.');
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let end = bytes.length - 22;
  for (; end >= Math.max(0, bytes.length - 65557); end--)
    if (view.getUint32(end, true) === 0x06054b50) break;
  if (
    end < Math.max(0, bytes.length - 65557) ||
    end + 22 + view.getUint16(end + 20, true) !== bytes.length
  )
    throw new Error('Keine vollständige XLSX-Datei.');
  const count = view.getUint16(end + 10, true);
  const centralEnd = view.getUint32(end + 16, true) + view.getUint32(end + 12, true);
  if (
    view.getUint16(end + 4, true) ||
    view.getUint16(end + 6, true) ||
    count > 512 ||
    centralEnd > end
  )
    throw new Error('Mehrteilige oder zu umfangreiche XLSX-Datei wird nicht unterstützt.');
  let offset = view.getUint32(end + 16, true),
    total = 0;
  const result = new Map<string, string>();
  for (let i = 0; i < count; i++) {
    if (offset + 46 > centralEnd || view.getUint32(offset, true) !== 0x02014b50)
      throw new Error('XLSX-Verzeichnis beschädigt.');
    const flags = view.getUint16(offset + 8, true),
      method = view.getUint16(offset + 10, true);
    const compressed = view.getUint32(offset + 20, true),
      size = view.getUint32(offset + 24, true);
    const length = view.getUint16(offset + 28, true),
      local = view.getUint32(offset + 42, true);
    const next =
      offset + 46 + length + view.getUint16(offset + 30, true) + view.getUint16(offset + 32, true);
    if (next > centralEnd || local + 30 > bytes.length) throw new Error('XLSX-Eintrag beschädigt.');
    const name = decoder.decode(bytes.subarray(offset + 46, offset + 46 + length));
    total += size;
    if (
      !name ||
      name.includes('\\') ||
      name.startsWith('/') ||
      name.split('/').some((p) => p === '..' || p === '.') ||
      result.has(name)
    )
      throw new Error('Unzulässiger oder doppelter XLSX-Dateipfad.');
    if (flags & 1 || ![0, 8].includes(method) || size > 12 * 1024 * 1024 || total > MAX_BYTES)
      throw new Error('Verschlüsselte oder zu große XLSX-Inhalte werden nicht eingelesen.');
    if (view.getUint32(local, true) !== 0x04034b50 || view.getUint16(local + 8, true) !== method)
      throw new Error('XLSX-Eintrag beschädigt.');
    const nameLength = view.getUint16(local + 26, true);
    const start = local + 30 + nameLength + view.getUint16(local + 28, true);
    if (
      start + compressed > view.getUint32(end + 16, true) ||
      decoder.decode(bytes.subarray(local + 30, local + 30 + nameLength)) !== name
    )
      throw new Error('XLSX-Dateigrenze ungültig.');
    const source = bytes.slice(start, start + compressed);
    let data: Uint8Array;
    if (method === 0) data = source;
    else {
      const stream = new Blob([source])
        .stream()
        .pipeThrough(new DecompressionStream('deflate-raw'));
      const reader = stream.getReader(),
        parts: Uint8Array[] = [];
      let received = 0;
      try {
        for (;;) {
          const chunk = await reader.read();
          if (chunk.done) break;
          received += chunk.value.length;
          if (received > size || received > 12 * 1024 * 1024)
            throw new Error('XLSX-Entpackgrenze überschritten.');
          parts.push(chunk.value);
        }
      } finally {
        await reader.cancel();
      }
      data = new Uint8Array(received);
      let position = 0;
      for (const part of parts) {
        data.set(part, position);
        position += part.length;
      }
    }
    if (data.length !== size || crc32(data) !== view.getUint32(offset + 16, true))
      throw new Error('XLSX-Prüfsumme oder Dateigröße ungültig.');
    // Only OOXML text is retained. Images, embedded objects and macros are never interpreted.
    result.set(name, /\.(xml|rels)$/i.test(name) ? decoder.decode(data) : '');
    offset = next;
  }
  return result;
}
export function zipFiles(files: { name: string; content: string }[]) {
  const encoder = new TextEncoder(),
    parts: Uint8Array[] = [],
    directory: Uint8Array[] = [];
  let offset = 0;
  for (const file of files) {
    const name = encoder.encode(file.name),
      data = encoder.encode(file.content),
      crc = crc32(data);
    if (!/^[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(file.name)) throw new Error('Exportname ungültig.');
    const local = new Uint8Array(30 + name.length),
      l = new DataView(local.buffer);
    l.setUint32(0, 0x04034b50, true);
    l.setUint16(4, 20, true);
    l.setUint16(6, 0x800, true);
    l.setUint32(14, crc, true);
    l.setUint32(18, data.length, true);
    l.setUint32(22, data.length, true);
    l.setUint16(26, name.length, true);
    local.set(name, 30);
    const central = new Uint8Array(46 + name.length),
      c = new DataView(central.buffer);
    c.setUint32(0, 0x02014b50, true);
    c.setUint16(4, 20, true);
    c.setUint16(6, 20, true);
    c.setUint16(8, 0x800, true);
    c.setUint32(16, crc, true);
    c.setUint32(20, data.length, true);
    c.setUint32(24, data.length, true);
    c.setUint16(28, name.length, true);
    c.setUint32(42, offset, true);
    central.set(name, 46);
    parts.push(local, data);
    directory.push(central);
    offset += local.length + data.length;
  }
  const centralSize = directory.reduce((sum, p) => sum + p.length, 0),
    end = new Uint8Array(22),
    e = new DataView(end.buffer);
  e.setUint32(0, 0x06054b50, true);
  e.setUint16(8, files.length, true);
  e.setUint16(10, files.length, true);
  e.setUint32(12, centralSize, true);
  e.setUint32(16, offset, true);
  if (files.length > 128 || offset + centralSize + 22 > 20 * 1024 * 1024)
    throw new Error('Zuordnungspaket überschreitet 20 MiB.');
  const output = new Uint8Array(offset + centralSize + 22);
  let position = 0;
  for (const part of [...parts, ...directory, end]) {
    output.set(part, position);
    position += part.length;
  }
  return output;
}
