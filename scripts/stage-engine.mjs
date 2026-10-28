import { copyFile, mkdir, access, cp, readdir, lstat, chmod, readFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { join, resolve } from 'node:path';
const output = resolve('desktop/resources/engine');
await mkdir(output, { recursive: true });
await copyFile('engine/target/inspect-engine-0.1.0.jar', join(output, 'inspect-engine.jar'));
const runtime = join(output, 'runtime');
async function checkRuntimeLicense(root) {
  const license = await readFile(join(root, 'legal/java.base/LICENSE'), 'utf8');
  const additional = await readFile(join(root, 'legal/java.base/ADDITIONAL_LICENSE_INFO'), 'utf8');
  if (!license.includes('GNU General Public License') || !additional.includes('Classpath'))
    throw new Error(
      'GPL/OpenJDK-Laufzeit mit Classpath Exception erforderlich. Frisches Buildverzeichnis und ein passendes JDK verwenden.',
    );
}
const executable = (name) =>
  process.env.JAVA_HOME
    ? join(process.env.JAVA_HOME, 'bin', name + (process.platform === 'win32' ? '.exe' : ''))
    : name;
const version = spawnSync(executable('java'), ['--version'], { encoding: 'utf8' });
if (version.status !== 0 || !/\b25(?:\.|\s)/.test(version.stdout))
  throw new Error('JDK 25 für die Zielplattform erforderlich. JAVA_HOME prüfen.');
try {
  await access(join(runtime, 'bin', process.platform === 'win32' ? 'java.exe' : 'java'));
} catch {
  const result = spawnSync(
    executable('jlink'),
    [
      '--add-modules',
      'java.base,java.xml,java.naming,jdk.crypto.ec',
      '--strip-debug',
      '--no-header-files',
      '--no-man-pages',
      '--output',
      runtime,
    ],
    { stdio: 'inherit' },
  );
  if (result.status !== 0)
    throw new Error('jlink fehlgeschlagen. JDK 25 für die Zielplattform erforderlich.');
}
await checkRuntimeLicense(runtime);
const release = await readFile(join(runtime, 'release'), 'utf8');
if (!/JAVA_VERSION="25(?:\.|")/.test(release))
  throw new Error('Vorhandene Laufzeit ist nicht Java 25. Frisches Buildverzeichnis verwenden.');
await cp('LICENSE', join(output, 'LICENSE-EUPL.txt'));
await cp('THIRD_PARTY_NOTICES.md', join(output, 'THIRD_PARTY_NOTICES.md'));
await cp('licenses', join(output, 'licenses'), { recursive: true });
// jlink legal files can be read-only. Tauri overwrites staged resources on later builds.
async function makeBuildResourcesWritable(directory) {
  for (const name of await readdir(directory)) {
    const file = join(directory, name),
      info = await lstat(file);
    if (info.isDirectory()) await makeBuildResourcesWritable(file);
    else if (info.isFile()) await chmod(file, info.mode | 0o200);
  }
}
await makeBuildResourcesWritable(output);
console.log('Diagnosekern und lokale Java-Laufzeit bereitgestellt.');
