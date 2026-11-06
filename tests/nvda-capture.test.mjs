import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';

test('NVDA observer preserves new focused labels while filtering unrelated controls and windows', () => {
  const result = spawnSync(
    process.platform === 'win32' ? 'python' : 'python3',
    ['tests/nvda_capture_test.py'],
    { encoding: 'utf8', timeout: 10000 },
  );
  assert.equal(result.status, 0, result.stdout + result.stderr);
});
