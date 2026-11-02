import test from 'node:test';
import assert from 'node:assert/strict';
import { checkCoverage } from '../scripts/quality/thresholds.mjs';
const valid = {
  total: { lines: { total: 10, pct: 95 }, branches: { total: 10, pct: 85 } },
  'module.ts': {},
};
test('coverage gates reject empty reports, missing modules and low or invalid metrics', () => {
  assert.doesNotThrow(() => checkCoverage(valid, ['module.ts']));
  assert.throws(() => checkCoverage({}, ['module.ts']), /Empty/);
  assert.throws(() => checkCoverage(valid, ['missing.ts']), /Missing/);
  for (const percent of [0, 79, NaN, 'Unknown'])
    assert.throws(
      () =>
        checkCoverage(
          { ...valid, total: { ...valid.total, branches: { total: 10, pct: percent } } },
          ['module.ts'],
        ),
      /branches/,
    );
});
