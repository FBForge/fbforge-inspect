import { execFileSync } from 'node:child_process';
import { readdir, readFile, mkdir, copyFile, writeFile, chmod } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
const output = resolve('licenses');
await mkdir(output, { recursive: true });
const inventory = [];
async function collect(root, name, version, license) {
  const safe = (name + '-' + version).replace(/[^a-zA-Z0-9._-]/g, '_');
  const destination = join(output, safe);
  await mkdir(destination, { recursive: true });
  const names = (await readdir(root)).filter((n) =>
    /^(licen[sc]e|copying|copyright|notice)([._-]|$)/i.test(n),
  );
  for (const file of names) {
    try {
      await copyFile(join(root, file), join(destination, file));
      await chmod(join(destination, file), 0o644);
    } catch (e) {
      if (e.code !== 'EISDIR') throw e;
    }
  }
  const item = {
    name,
    version,
    license: license || 'See upstream distribution',
    licenseFiles: names,
  };
  inventory.push(item);
  await writeFile(join(destination, 'COMPONENT.json'), JSON.stringify(item, null, 2) + '\n');
}
const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm';
// npm.cmd requires a command interpreter on Windows. No untrusted values are interpolated.
const paths = execFileSync(npm, ['ls', '--omit=dev', '--all', '--parseable'], {
  encoding: 'utf8',
  shell: process.platform === 'win32',
})
  .trim()
  .split(/\r?\n/)
  .slice(1);
for (const path of new Set(paths)) {
  const p = JSON.parse(await readFile(join(path, 'package.json'), 'utf8'));
  await collect(path, p.name, p.version, p.license);
}
const target = process.platform === 'win32' ? 'x86_64-pc-windows-msvc' : 'x86_64-unknown-linux-gnu';
const cargo = JSON.parse(
  execFileSync(
    'cargo',
    [
      'metadata',
      '--locked',
      '--offline',
      '--filter-platform',
      target,
      '--format-version',
      '1',
      '--manifest-path',
      'desktop/Cargo.toml',
    ],
    { encoding: 'utf8', maxBuffer: 30 * 1024 * 1024 },
  ),
);
for (const p of cargo.packages)
  if (p.name !== 'fbforge-inspect')
    await collect(dirname(p.manifest_path), p.name, p.version, p.license);
inventory.sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version));
await writeFile(join(output, 'components.json'), JSON.stringify(inventory, null, 2) + '\n');
await writeFile(
  join(output, `components.${target}.json`),
  JSON.stringify(inventory, null, 2) + '\n',
);
console.log(
  `${inventory.length} frontend/Rust component notices collected for ${target}. Java notices are in licenses/java/.`,
);
