import test from 'node:test';
import assert from 'node:assert/strict';
import { validRegion } from '../frontend/src/areaKeys.ts';
import {
  checksFromText,
  parseJob,
  parseResult,
  recognizeInput,
  exitCode,
} from '../frontend/src/portable.ts';
import { certificateInventory, renewalCalendar } from '../frontend/src/inventory.ts';
import { supportText, explainHttp } from '../frontend/src/support.ts';
import { addressingContexts } from '../frontend/src/addressing.ts';
import { serverNetworkEvidence } from '../frontend/src/serverEvidence.ts';
const id = '00000000-0000-4000-8000-000000000001';
const finding = {
  status: 'unknown',
  title: 'TLS-Verbindung',
  detail: 'Vertrauen nicht bestätigt.',
  path: '$',
  source: 'https://example.org/',
  nextStep: 'HTTPS-CA prüfen.',
  limitation: '',
};
function report() {
  return {
    protocolVersion: 1,
    appVersion: '0.2.0',
    createdAt: '2026-11-06T12:00:00Z',
    operation: 'destination',
    environment: 'TEST',
    apiVersion: 'v3',
    findings: [finding],
    requests: [],
    data: {},
  };
}
function result() {
  return {
    format: 'fbforge-inspect-result',
    formatVersion: 1,
    appVersion: '0.2.0',
    createdAt: '2026-11-06T12:00:00Z',
    runtime: { os: 'Linux', architecture: 'aarch64', javaVersion: '25' },
    checks: [{ id: 'one', report: report() }],
    exitCode: 3,
  };
}
function job() {
  return {
    format: 'fbforge-inspect-job',
    formatVersion: 1,
    environment: 'TEST',
    apiVersion: 'v3',
    checks: [{ id: 'one', operation: 'destination', input: { destinationId: id } }],
  };
}

test('smart paste recognizes official context and never fetches the supplied URL', () => {
  assert.deepEqual(recognizeInput(id), { operation: 'destination', input: { destinationId: id } });
  const context = recognizeInput(
    `https://prod.fit-connect.fitko.net/submission-api/v3/destinations/${id}`,
  );
  assert.equal(context.environment, 'PROD');
  assert.equal(context.apiVersion, 'v3');
  assert.equal(
    recognizeInput(
      'https://routing-api-testing.fit-connect.fitko.dev/v2/routes?leikaKey=99008001014002&ars=020000000000',
    ).input.regionKind,
    'ars',
  );
  for (const url of [
    `https://evil.example/submission-api/v3/destinations/${id}`,
    `https://user:pass@prod.fit-connect.fitko.net/submission-api/v3/destinations/${id}`,
    `http://prod.fit-connect.fitko.net/submission-api/v3/destinations/${id}`,
    `https://prod.fit-connect.fitko.net/submission-api/v3/destinations/${id}?token=synthetic`,
    'https://routing-api-testing.fit-connect.fitko.dev/v2/routes?leikaKey=99008001014002&ars=short',
  ])
    assert.equal(recognizeInput(url), null);
});
test('batch inputs are bounded and keep routing tuples together', () => {
  const checks = checksFromText(`${id}\nrouting,99008001014002,020000000000\nnetwork`);
  assert.equal(checks.length, 3);
  assert.deepEqual(checks[1].input, {
    service: '99008001014002',
    region: '020000000000',
    regionKind: 'ars',
    pvog: false,
  });
  assert.throws(() => checksFromText(Array(33).fill(id).join('\n')));
  assert.throws(() => checksFromText(''));
  assert.throws(() => checksFromText('unknown'));
});

test('routing paste and batch preserve every ARS level including leading zeros', () => {
  for (const digits of ['02', '020', '02000', '020000000', '020000000000']) {
    for (const prefix of ['', 'DE']) {
      const region = prefix + digits;
      const url = `https://routing-api-testing.fit-connect.fitko.dev/v2/routes?leikaKey=99008001014002&ars=${region}`;
      assert.equal(validRegion('ars', region), true);
      assert.equal(recognizeInput(url).input.region, region);
      assert.equal(checksFromText(url)[0].input.region, region);
      assert.deepEqual(checksFromText(`routing,99008001014002,${region}`)[0].input, {
        service: '99008001014002',
        region,
        regionKind: 'ars',
        pvog: false,
      });
    }
  }
});

test('invalid ARS lengths and characters are rejected without confusing AGS or area IDs', () => {
  for (const region of [
    '',
    '0',
    '0200',
    '020000',
    '0200000',
    '02000000',
    '0200000000',
    '02000000000',
    '0200000000000',
    '０２',
    '0A',
    '-2',
    '02 00',
    'DEDE02',
  ]) {
    assert.equal(validRegion('ars', region), false, region);
    assert.equal(
      recognizeInput(
        `https://routing-api-testing.fit-connect.fitko.dev/v2/routes?leikaKey=99008001014002&ars=${encodeURIComponent(region)}`,
      ),
      null,
    );
    assert.throws(() => checksFromText(`routing,99008001014002,${region}`));
  }
  assert.equal(validRegion('ars', '02\n'), false);
  assert.equal(validRegion('ags', '02000000'), true);
  assert.equal(validRegion('ags', 'DE02000000'), true);
  assert.equal(validRegion('ags', '02'), false);
  assert.equal(validRegion('areaId', '1'), true);
  assert.equal(validRegion('unknown', '02'), false);
});
test('portable jobs reject credentials and duplicate work identifiers', () => {
  assert.equal(parseJob(JSON.stringify(job())).checks.length, 1);
  for (const field of ['password', 'username', 'authorization']) {
    const input = job();
    input.network = {
      mode: 'manual',
      proxyUrl: 'https://proxy.example:8080',
      [field]: 'synthetic',
    };
    assert.throws(() => parseJob(JSON.stringify(input)));
  }
  const duplicate = job();
  duplicate.checks.push(duplicate.checks[0]);
  assert.throws(() => parseJob(JSON.stringify(duplicate)));
  const input = job();
  input.network = { mode: 'manual', proxyUrl: 'https://user:pass@proxy.example' };
  assert.throws(() => parseJob(JSON.stringify(input)));
  const privateInput = job();
  privateInput.checks[0] = {
    id: 'private',
    operation: 'keys',
    input: { text: '{"kty":"RSA","d":"synthetic-private-marker"}' },
  };
  assert.throws(() => parseJob(JSON.stringify(privateInput)));
});
test('addressing explanations retain separate authority contexts and never equate signature validity with context', () => {
  const input = report();
  input.data = {
    routes: [{ destinationId: id }],
    pvogRelations: [
      {
        uebergeordnetesObjektID: { value: 'OE-A' },
        zustaendigkeit: [{ leistungID: [{ value: 'LB' }], gebietID: [{ value: 'ARS' }] }],
      },
      {
        uebergeordnetesObjektID: { value: 'OE-B' },
        zustaendigkeit: [{ leistungID: [{ value: 'LB' }], gebietID: [{ value: 'ARS' }] }],
      },
    ],
    addressingSignatures: [
      {
        context: '$.pvogRelations[0].zustaendigkeit[0].idSekundaer[0]',
        verified: true,
        contextMatches: true,
        claims: { destinationId: id },
      },
      {
        context: '$.pvogRelations[1].zustaendigkeit[0].idSekundaer[0]',
        verified: true,
        contextMatches: false,
        claims: { destinationId: id },
      },
    ],
  };
  const rows = addressingContexts(input);
  assert.equal(rows.length, 2);
  assert.equal(rows[0].parent, 'OE-A');
  assert.equal(rows[1].parent, 'OE-B');
  assert.equal(rows[0].addresses[0].inRouting, true);
  assert.equal(rows[0].addresses[0].contextMatches, true);
  assert.equal(rows[1].addresses[0].verified, true);
  assert.equal(rows[1].addresses[0].contextMatches, false);
});
test('server result import is versioned and strips raw responses', () => {
  const input = result();
  input.checks[0].report.rawResponses = { example: 'public body' };
  const imported = parseResult(JSON.stringify(input));
  assert.equal(imported.runtime.os, 'Linux');
  assert.equal(imported.checks[0].report.rawResponses, undefined);
  for (const replacement of [0, 2, '1']) {
    const invalid = result();
    invalid.formatVersion = replacement;
    assert.throws(() => parseResult(JSON.stringify(invalid)));
  }
});
test('optional server transport fields are normalized before rendering and never invent timing', () => {
  const input = report();
  input.data = {
    networkChecks: [null, { name: 'Remote probe', durationMs: 'not measured', httpStatus: -1 }],
    networkTraces: [
      {
        url: 'https://example.org/',
        route: 'direct',
        phases: [null, { stage: 'tls', durationMs: 'invalid', outcome: 'TLS_FAILED' }],
        tls: { cipher: {}, certificates: [null, { subject: '<script>inert</script>' }] },
      },
    ],
    networkContext: {
      httpsTrust: 'Transferred trust description',
      password: 'synthetic-unrelated-field',
    },
  };
  const safe = serverNetworkEvidence(input);
  assert.equal(safe.data.networkChecks.length, 1);
  assert.equal(safe.data.networkChecks[0].durationMs, undefined);
  assert.equal(safe.data.networkChecks[0].httpStatus, undefined);
  assert.equal(safe.data.networkTraces[0].tls.cipher, '');
  assert.equal(safe.data.networkTraces[0].phases.length, 1);
  assert.equal(safe.data.networkContext.password, undefined);
  assert.match(
    supportText(input, 'it', true, { os: 'Linux', architecture: 'aarch64', javaVersion: '25' }),
    /Übermittelter Laufzeitkontext: Linux/,
  );
});
test('malformed imported findings cannot reach the renderer', () => {
  for (const field of ['title', 'detail', 'path', 'source', 'nextStep', 'limitation']) {
    const input = result();
    input.checks[0].report.findings = [{ ...finding, [field]: {} }];
    assert.throws(() => parseResult(JSON.stringify(input)));
  }
  const bad = result();
  bad.checks[0].report.findings = [{ ...finding, status: 'green' }];
  assert.throws(() => parseResult(JSON.stringify(bad)));
  const privateKey = result();
  privateKey.checks[0].report.data = { kty: 'RSA', d: 'synthetic' };
  assert.throws(() => parseResult(JSON.stringify(privateKey)));
});
test('unknown checks dominate errors while warnings remain an explicit completed result', () => {
  assert.equal(exitCode([{ id: 'a', error: 'CHECK_FAILED' }]), 3);
  assert.equal(
    exitCode([{ id: 'a', report: { ...report(), findings: [{ ...finding, status: 'error' }] } }]),
    2,
  );
  assert.equal(
    exitCode([{ id: 'a', report: { ...report(), findings: [{ ...finding, status: 'warning' }] } }]),
    0,
  );
});
test('certificate inventory groups shared certificates and preserves destination contexts', () => {
  const base = {
    ...report(),
    data: {
      destination: { destinationId: id, encryptionKid: 'old' },
      certificates: [
        {
          sha256: 'a'.repeat(64),
          kid: 'old',
          chainIndex: 0,
          subject: 'Synthetic',
          issuer: 'Fixture CA',
          notBefore: '2026-01-01T00:00:00Z',
          notAfter: '2026-12-01T00:00:00Z',
        },
        {
          sha256: 'b'.repeat(64),
          chainIndex: 1,
          notBefore: '2026-01-01T00:00:00Z',
          notAfter: '2026-12-01T00:00:00Z',
        },
      ],
    },
  };
  const rows = certificateInventory(
    [base, base, { ...base, environment: 'PROD' }],
    Date.parse('2026-11-01T00:00:00Z'),
  );
  assert.equal(rows.length, 1);
  assert.equal(rows[0].uses.length, 2);
  assert.equal(rows[0].days, 30);
  assert.equal(rows[0].uses[0].active, true);
  const calendar = renewalCalendar(rows, 30);
  assert.match(calendar, /DTSTART;VALUE=DATE:20261101/);
  assert.match(calendar, /BEGIN:VCALENDAR/);
  assert.equal(
    certificateInventory([{ ...base, data: { certificates: [{ sha256: {}, chainIndex: 0 }] } }])
      .length,
    0,
  );
});
test('support preview defaults to omitting source fields and explains limits', () => {
  assert.doesNotMatch(supportText(report(), 'it', false), /https:\/\/example.org/);
  assert.match(supportText(report(), 'it', true), /https:\/\/example.org/);
  assert.match(supportText(report(), 'fitko', false), /keine nachgewiesene Fehlerzuweisung/);
  assert.match(explainHttp(407), /Proxy/);
  assert.match(explainHttp(429), /Ratenlimit/);
  assert.match(explainHttp(200), /eigene Prüfungen/);
});
