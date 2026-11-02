import test from 'node:test';
import assert from 'node:assert/strict';
import { ref } from 'vue';
import { useWorkspace } from '../frontend/src/useWorkspace.ts';

function workspace() {
  const page = ref('destination'),
    environment = ref('TEST'),
    version = ref('v3');
  return { page, environment, version, ...useWorkspace(page, environment, version) };
}

test('task, environment and API changes retain isolated inputs and results', () => {
  const w = workspace();
  w.session.value.form.destinationId = 'test-v3';
  w.session.value.report = { data: { marker: 'retained' } };
  w.session.value.scrollTop = 230;
  const original = w.session.value;
  w.page.value = 'routing';
  w.session.value.form.service = '99008001014002';
  w.page.value = 'destination';
  w.environment.value = 'PROD';
  assert.equal(w.session.value.form.destinationId, '');
  w.environment.value = 'TEST';
  w.version.value = 'v2';
  assert.equal(w.session.value.report, null);
  w.version.value = 'v3';
  assert.equal(w.session.value, original);
  assert.equal(w.session.value.report.data.marker, 'retained');
  assert.equal(w.session.value.scrollTop, 230);
  w.page.value = 'routing';
  assert.equal(w.session.value.form.service, '99008001014002');
});

test('process search and independent limits retain separate drafts', () => {
  const w = workspace();
  w.page.value = 'process';
  w.session.value.form.messageId = 'message';
  w.processMode.value = 'limits';
  w.session.value.form.fromDestinationId = 'sender';
  assert.equal(w.session.value.form.messageId, '');
  w.processMode.value = 'process';
  assert.equal(w.session.value.form.messageId, 'message');
  assert.equal(w.session.value.form.fromDestinationId, '');
  w.processMode.value = 'limits';
  assert.equal(w.session.value.form.fromDestinationId, 'sender');
});

test('schema references keep the original work and navigation stays inside the history', () => {
  const w = workspace();
  w.page.value = 'schema';
  w.session.value.form.text = '{"title":"original"}';
  w.prepareSchemaReference();
  w.session.value.form.url = 'https://example.org/reference';
  assert.equal(w.schemaTrail.value.keys.length, 2);
  w.moveSchema(-1);
  assert.equal(w.session.value.form.text, '{"title":"original"}');
  w.moveSchema(-1);
  assert.equal(w.schemaTrail.value.index, 0);
  w.moveSchema(1);
  assert.equal(w.session.value.form.url, 'https://example.org/reference');
  w.moveSchema(1);
  assert.equal(w.schemaTrail.value.index, 1);
  w.environment.value = 'PROD';
  assert.equal(w.schemaTrail.value.keys.length, 1);
  w.environment.value = 'TEST';
  assert.equal(w.session.value.form.url, 'https://example.org/reference');
});
