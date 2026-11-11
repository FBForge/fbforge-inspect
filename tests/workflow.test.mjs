import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { resultState, jobDigest } from '../frontend/src/workflow.ts';
test('completed reports retain errors and missing evidence instead of implying success', () => {
  const report = (statuses) => ({ findings: statuses.map((status) => ({ status })) });
  assert.equal(resultState(), 'pending');
  assert.equal(resultState({ error: 'CHECK_FAILED' }), 'unknown');
  assert.equal(resultState({ report: report([]) }), 'unknown');
  assert.equal(resultState({ report: report(['confirmed', 'unknown']) }), 'unknown');
  assert.equal(resultState({ report: report(['error', 'unknown']) }), 'error');
  assert.equal(resultState({ report: report(['warning', 'confirmed']) }), 'warning');
  assert.equal(resultState({ report: report(['notApplicable']) }), 'notApplicable');
});
test('job association binds exact UTF-8 bytes, including formatting differences', async () => {
  const bytes = new TextEncoder().encode('{"name":"Prüfung"}');
  assert.equal(await jobDigest(bytes), createHash('sha256').update(bytes).digest('hex'));
  assert.notEqual(
    await jobDigest(bytes),
    await jobDigest(new TextEncoder().encode('{"name":"Prüfung"}\n')),
  );
});
