import test from 'node:test';
import assert from 'node:assert/strict';
import { containsPrivateMaterial } from '../frontend/src/inputs.ts';
test('actual JWK secrets are rejected across input workflows', () => {
  assert.ok(containsPrivateMaterial('{"d":"partial-private-fixture"}', true));
  assert.ok(containsPrivateMaterial('{"keys":[{"kty":"RSA","d":"private-fixture"}]}'));
  assert.ok(containsPrivateMaterial('{"kty":"oct","k":"symmetric-fixture"}'));
  assert.ok(containsPrivateMaterial('-----BEGIN PRIVATE KEY-----'));
});
test('schema property names do not become private keys', () => {
  assert.equal(
    containsPrivateMaterial('{"properties":{"kty":{"type":"string"},"d":{"type":"string"}}}'),
    false,
  );
  assert.equal(containsPrivateMaterial('{"kty":"RSA","n":"public","e":"AQAB"}'), false);
});
