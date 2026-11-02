import test from 'node:test';
import assert from 'node:assert/strict';
import { createNetworkSettings, networkRequest } from '../frontend/src/network.ts';

test('disabling proxy excludes dormant credentials from diagnosis requests', () => {
  const settings = createNetworkSettings();
  settings.mode = 'manual';
  settings.proxyUrl = ' https://proxy.example:8080 ';
  settings.username = 'fixture-user';
  settings.password = 'fixture-password';
  assert.deepEqual(networkRequest(settings), {
    mode: 'manual',
    proxyUrl: 'https://proxy.example:8080',
    username: 'fixture-user',
    password: 'fixture-password',
  });
  settings.mode = 'direct';
  assert.deepEqual(networkRequest(settings), { mode: 'direct' });
});
