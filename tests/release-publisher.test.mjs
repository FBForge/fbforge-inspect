import test from 'node:test';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

test('release promotion requires exact successful provenance and an intact signed package', () => {
  execFileSync(
    process.platform === 'win32' ? 'python' : 'python3',
    ['-B', fileURLToPath(new URL('./release_publisher_test.py', import.meta.url))],
    { encoding: 'utf8', timeout: 10000 },
  );
});
