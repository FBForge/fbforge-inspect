import test from 'node:test';
import assert from 'node:assert/strict';
import { reportForExport, reportHtml } from '../frontend/src/reports.ts';
import { reactive } from 'vue';
const report = {
  protocolVersion: 1,
  appVersion: '0.1.0',
  createdAt: '2026-10-28',
  operation: 'schema',
  environment: 'TEST',
  apiVersion: 'v3',
  findings: [],
  requests: [],
  data: { value: '</pre><script>alert(1)</script><img src="https://attacker.example/a">' },
  rawResponses: { 'https://example.org': 'optional raw' },
};
test('raw responses are excluded by default without modifying session data', () => {
  assert.equal(reportForExport(report).rawResponses, undefined);
  assert.ok(report.rawResponses);
  assert.ok(reportForExport(report, true).rawResponses);
});
test('standalone reports render remote markup inert and block external resources', () => {
  const html = reportHtml(reportForExport(report));
  assert.ok(!html.includes('<script>'));
  assert.ok(!html.includes('<img '));
  assert.ok(html.includes('&lt;script&gt;'));
  assert.ok(html.includes("default-src 'none'"));
  assert.ok(!html.includes('optional raw'));
});
test('export accepts a reactive session report', () => {
  assert.equal(reportForExport(reactive(report)).environment, 'TEST');
});
