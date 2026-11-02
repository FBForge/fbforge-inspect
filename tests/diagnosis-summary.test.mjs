import test from 'node:test';
import assert from 'node:assert/strict';
import { diagnosisSummary } from '../frontend/src/diagnosisSummary.ts';
import { reportHtml } from '../frontend/src/reports.ts';
import { successfulDiagnosis, summaryScenarios, check } from './fixtures/diagnosis-summary.ts';

for (const { name, report, title, text } of summaryScenarios()) {
  test(`diagnosis conclusion: ${name}`, () => {
    const result = diagnosisSummary(report);
    assert.equal(result.title, title);
    assert.ok(JSON.stringify(result).includes(text));
    const html = reportHtml(report);
    assert.ok(html.includes(title));
    assert.ok(html.includes(text));
    report.findings.forEach((f) => {
      f.title = 'Anders benannt';
      f.detail = 'Keine Auswertung dieses Textes';
    });
    assert.deepEqual(diagnosisSummary(report), result);
  });
}

test('a custom anchor must not become a V-PKI claim', () => {
  const report = successfulDiagnosis();
  report.data.certificateTrust[0].source = 'additional-ca';
  assert.ok(!JSON.stringify(diagnosisSummary(report)).includes('V-PKI'));
});

test('positive counts cannot hide missing categories or partial revocation coverage', () => {
  for (const code of [
    'JWK_REQUIREMENTS',
    'CERTIFICATE_VALIDITY',
    'CERTIFICATE_TRUST',
    'CRL_STATUS',
    'DESTINATION_STATUS',
  ]) {
    const report = successfulDiagnosis();
    report.findings = report.findings.filter((f) => f.code !== code);
    assert.equal(diagnosisSummary(report).title, 'Prüfung mit offenen Punkten abgeschlossen', code);
  }
  const report = successfulDiagnosis();
  report.findings = [check('DESTINATION_ID')];
  assert.equal(diagnosisSummary(report).title, 'Prüfung mit offenen Punkten abgeschlossen');
});

test('a second unverified key prevents a blanket success', () => {
  const report = successfulDiagnosis();
  report.data.keys.push({ kid: 'second', x5cCount: 0 });
  assert.equal(diagnosisSummary(report).title, 'Prüfung mit offenen Punkten abgeschlossen');
  report.findings.push(check('KEY_DATA_INVALID', 'error'));
  assert.equal(diagnosisSummary(report).items.find((i) => i.id === 'keys').status, 'error');
});

test('unknown general findings still prevent a happy headline and HTML stays escaped', () => {
  const report = successfulDiagnosis();
  report.findings.push({ ...check('GENERAL', 'unknown'), title: '<img src=x onerror=alert(1)>' });
  assert.equal(diagnosisSummary(report).title, 'Prüfung mit offenen Punkten abgeschlossen');
  assert.ok(!reportHtml(report).includes('<img src=x'));
  assert.ok(reportHtml(report).includes('&lt;img src=x'));
  report.findings.at(-1).status = 'error';
  assert.equal(
    diagnosisSummary(report).message,
    'Weitere Einzelbefunde enthalten Fehler oder Abweichungen.',
  );
  report.operation = 'routing';
  assert.equal(diagnosisSummary(report), null);
});

test('missing PROD chains remain errors and non-applicable keys are not certified', () => {
  const report = summaryScenarios().find((s) => s.name === 'TEST ohne x5c').report;
  report.environment = 'PROD';
  report.findings.find((f) => f.code === 'CERTIFICATE_CHAIN').status = 'error';
  assert.equal(diagnosisSummary(report).title, 'Abweichungen gefunden');
  report.data.keys = [];
  report.data.certificates = [];
  report.findings = [
    check('DESTINATION_ID'),
    check('DESTINATION_STATUS'),
    check('DESTINATION_CONFIGURATION'),
    check('JWK_REQUIREMENTS', 'notApplicable'),
  ];
  const result = diagnosisSummary(report);
  assert.equal(result.items.find((i) => i.id === 'keys').status, 'notApplicable');
  assert.ok(!result.message.includes('Zertifikatskette'));
});
