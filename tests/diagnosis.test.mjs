import test from 'node:test';
import assert from 'node:assert/strict';
import { findingDetail, findingNext } from '../frontend/src/diagnosis.ts';
import { reportHtml } from '../frontend/src/reports.ts';

const finding = {
  code: 'DESTINATION_STATUS',
  status: 'error',
  title: 'Umformulierter Titel',
  detail: 'inactive',
  path: '$.status',
  source: '',
  nextStep: 'Quelle oder Konfiguration gezielt prüfen.',
  limitation: '',
};

test('finding behavior follows its code even when its visible title changes', () => {
  assert.equal(findingDetail(finding), 'Der Zustellpunkt ist inaktiv.');
  assert.equal(findingNext(finding), 'Status beim Betreiber des Zustellpunkts klären.');
  assert.equal(
    findingDetail({ ...finding, code: 'GENERAL', title: 'Destination-Status' }),
    'inactive',
  );
});

test('actionable guidance remains independent of translated finding titles', () => {
  for (const [code, expected] of [
    ['DESTINATION_ID', 'Destination-ID und Umgebung'],
    ['TRUST_ANCHOR_MISSING', 'mitgelieferten V-PKI-Truststore'],
    ['REVOCATION_NOT_REQUESTED', 'OCSP-/CRL-Prüfung'],
    ['SCHEMA_REFERENCES_UNRESOLVED', 'Das Ausgangsschema bleibt erhalten.'],
    ['PVOG_INPUT_REQUIRED', '12-stelligen ARS'],
  ]) {
    assert.ok(findingNext({ ...finding, code }).includes(expected), code);
  }
});

test('export uses the same code-based interpretation as the application', () => {
  const html = reportHtml({
    protocolVersion: 1,
    appVersion: '0.1.0',
    createdAt: '2026-11-02',
    operation: 'destination',
    environment: 'TEST',
    apiVersion: 'v3',
    findings: [finding],
    requests: [],
    data: {},
  });
  assert.ok(html.includes('Der Zustellpunkt ist inaktiv.'));
  assert.ok(html.includes('Status beim Betreiber des Zustellpunkts klären.'));
});
