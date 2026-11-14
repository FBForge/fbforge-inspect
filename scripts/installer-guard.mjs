import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { copyFile, lstat, mkdir, open } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const target = 'i686-pc-windows-msvc';
const targetDir = join(root, 'desktop/target/installer-guard');
const output = join(root, 'desktop/installer/guard/fbforge_suite_nsis_guard.dll');

async function readGuard(path) {
  const stat = await lstat(path);
  if (!stat.isFile() || stat.size < 1 || stat.size > 32 * 1024 * 1024) {
    throw new Error('guard_file_invalid');
  }
  const file = await open(path, 'r');
  try {
    const bytes = Buffer.alloc(stat.size + 1);
    let count = 0;
    while (count < bytes.length) {
      const { bytesRead } = await file.read(bytes, count, bytes.length - count, null);
      if (!bytesRead) break;
      count += bytesRead;
    }
    if (count !== stat.size) throw new Error('guard_file_changed');
    return bytes.subarray(0, count);
  } finally {
    await file.close();
  }
}

try {
  const args = process.argv.slice(2);
  if (args[0] === 'verify' && args.length === 2 && /^[a-f0-9]{64}$/.test(args[1])) {
    // The protected signer supplied this hash after Authenticode verification.
    // Bundling must preserve those bytes; this branch never builds or copies.
    const hash = createHash('sha256')
      .update(await readGuard(output))
      .digest('hex');
    if (hash !== args[1]) throw new Error('signed_guard_changed');
    console.log('Verified the staged guard bytes; no rebuild performed.');
  } else if (args.length === 0) {
    if (process.platform !== 'win32') throw new Error('guard_build_requires_windows');
    const result = spawnSync(
      'rustup',
      [
        'run',
        '1.93.1',
        'cargo',
        'build',
        '--manifest-path',
        join(root, 'desktop/installer/guard-source/Cargo.toml'),
        '--locked',
        '--release',
        '--target',
        target,
        '-p',
        'fbforge-suite-nsis-guard',
        '--lib',
      ],
      {
        cwd: root,
        env: {
          ...process.env,
          CARGO_TARGET_DIR: targetDir,
          CARGO_ENCODED_RUSTFLAGS: '-C\x1ftarget-feature=+crt-static',
        },
        stdio: 'inherit',
      },
    );
    if (result.status !== 0)
      throw new Error('guard_build_failed_check_installed_toolchain_and_target');
    const built = join(targetDir, target, 'release/fbforge_suite_nsis_guard.dll');
    await readGuard(built);
    await mkdir(dirname(output), { recursive: true });
    await copyFile(built, output);
    console.log('Staged the unsigned x86 guard. This is not a trusted release installer.');
  } else {
    throw new Error('usage: installer-guard.mjs [verify <sha256>]');
  }
} catch (error) {
  console.error(error.code || error.message);
  process.exitCode = 1;
}
