import { cp, mkdir, readFile, readdir, writeFile, rm } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
const output = 'dist-website';
await rm(`${output}/site`, { recursive: true, force: true });
await mkdir(`${output}/site/inspect/images`, { recursive: true });
await cp('website/inspect', `${output}/site/inspect`, { recursive: true });
await cp('docs/images', `${output}/site/inspect/images`, { recursive: true });
await cp('dist-help/inspect/hilfe', `${output}/site/inspect/hilfe`, { recursive: true });
const files = [];
async function inventory(directory) {
  for (const entry of await readdir(`${output}/site/${directory}`, { withFileTypes: true })) {
    const path = `${directory}/${entry.name}`;
    if (entry.isDirectory()) await inventory(path);
    else if (entry.isFile())
      files.push({
        path,
        sha256: createHash('sha256')
          .update(await readFile(`${output}/site/${path}`))
          .digest('hex'),
      });
    else throw new Error(`Unsupported export entry: ${path}`);
  }
}
await inventory('inspect');
const version = JSON.parse(await readFile('package.json', 'utf8')).version;
const manifest = {
  schemaVersion: 1,
  product: 'FBForge Inspect',
  version,
  sourceCommit: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
  sourceDirty:
    execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim().length > 0,
  files: files.sort((a, b) => a.path.localeCompare(b.path)),
};
await writeFile(`${output}/manifest.json`, JSON.stringify(manifest, null, 2) + '\n');
console.log(`Website export: ${files.length} public files; sourceDirty=${manifest.sourceDirty}`);
