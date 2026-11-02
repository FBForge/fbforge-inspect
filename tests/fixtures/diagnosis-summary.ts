import type { Finding, FindingCode, Report, Status } from '../../frontend/src/contracts.ts';

export function check(code: FindingCode, status: Status = 'confirmed', path = '$'): Finding {
  return {
    code,
    status,
    path,
    title: 'Unabhängig vom Titel',
    detail: '',
    source: '',
    nextStep: '',
    limitation: '',
  };
}

/** Synthetic diagnostic evidence, not a certificate or network fixture. */
export function successfulDiagnosis(): Report {
  return {
    protocolVersion: 1,
    appVersion: '0.1.0',
    createdAt: '2026-11-02T16:00:00Z',
    operation: 'destination',
    environment: 'PROD',
    apiVersion: 'v3',
    requests: [],
    findings: [
      check('DESTINATION_ID'),
      check('DESTINATION_STATUS'),
      check('DESTINATION_CONFIGURATION'),
      ...['kty', 'kid', 'bits', 'e', 'alg', 'key_ops', 'use'].map((field) =>
        check('JWK_REQUIREMENTS', 'confirmed', '$.keys[0].' + field),
      ),
      check('CERTIFICATE_BINDING'),
      ...[0, 1, 2].map((index) =>
        check('CERTIFICATE_VALIDITY', 'confirmed', `$.keys[0].x5c[${index}]`),
      ),
      check('CERTIFICATE_SIGNATURE'),
      check('CERTIFICATE_TRUST'),
      check('OCSP_STATUS', 'confirmed', '$.keys[0].x5c[0]'),
      check('CRL_STATUS', 'confirmed', '$.keys[0].x5c[0]'),
      check('CRL_STATUS', 'confirmed', '$.keys[0].x5c[1]'),
    ],
    data: {
      destination: {
        destinationId: '85a29591-22a6-49f3-9ac6-e31158aba022',
        status: 'active',
        type: 'A',
        encryptionKid: 'public-key',
        metadataVersions: ['2.0.0'],
      },
      keys: [{ kid: 'public-key', algorithm: 'RSA-OAEP-256', bits: 4096, x5cCount: 3 }],
      certificates: [
        { subject: 'CN=Teilnehmer', issuer: 'CN=Aussteller', chainIndex: 0 },
        { subject: 'CN=Aussteller', issuer: 'CN=Wurzel', chainIndex: 1 },
        { subject: 'CN=Wurzel', issuer: 'CN=Wurzel', chainIndex: 2 },
      ],
      certificateTrust: [{ trusted: true, source: 'bundled-vpki', chain: [{}, {}, {}] }],
    },
  };
}

export function summaryScenarios() {
  const happy = successfulDiagnosis();
  const v2 = successfulDiagnosis();
  v2.apiVersion = 'v2';
  v2.findings.push(check('DESTINATION_CONFIGURATION', 'notApplicable'));
  const crlOffline = successfulDiagnosis();
  crlOffline.findings
    .filter((f) => f.code === 'CRL_STATUS')
    .forEach((f) => {
      f.status = 'unknown';
      f.detail = 'HTTP 500';
    });
  const testKey = successfulDiagnosis();
  testKey.environment = 'TEST';
  testKey.data.certificates = [{ subject: 'CN=TEST', issuer: 'CN=TEST' }];
  testKey.data.keys![0]!.x5cCount = 1;
  testKey.data.certificateTrust = [{ trusted: false, source: 'test-unanchored' }];
  testKey.findings = testKey.findings.filter(
    (f) =>
      !['CRL_STATUS', 'OCSP_STATUS'].includes(f.code!) &&
      !(f.code === 'CERTIFICATE_VALIDITY' && !f.path.endsWith('[0]')),
  );
  testKey.findings.find((f) => f.code === 'CERTIFICATE_TRUST')!.status = 'warning';
  testKey.findings.push(check('REVOCATION_UNAVAILABLE', 'warning'));
  const noChain = structuredClone(testKey);
  noChain.data.certificates = [];
  noChain.data.keys![0]!.x5cCount = 0;
  noChain.findings = noChain.findings.filter(
    (f) => !f.code!.startsWith('CERTIFICATE_') && f.code !== 'REVOCATION_UNAVAILABLE',
  );
  noChain.findings.push(check('CERTIFICATE_CHAIN', 'warning'));
  const revoked = successfulDiagnosis();
  revoked.findings.find((f) => f.code === 'OCSP_STATUS')!.status = 'error';
  const expired = successfulDiagnosis();
  expired.findings.find((f) => f.code === 'CERTIFICATE_VALIDITY')!.status = 'error';
  const offline = successfulDiagnosis();
  offline.findings = offline.findings.filter(
    (f) => !['CRL_STATUS', 'OCSP_STATUS'].includes(f.code!),
  );
  offline.findings.push(check('REVOCATION_NOT_REQUESTED', 'unknown'));
  const partial = successfulDiagnosis();
  partial.findings = partial.findings.filter((f) => f.code !== 'CERTIFICATE_BINDING');
  const standalone = successfulDiagnosis();
  standalone.operation = 'keys';
  const inactive = successfulDiagnosis();
  inactive.data.destination!.status = 'inactive';
  inactive.findings.find((f) => f.code === 'DESTINATION_STATUS')!.status = 'error';
  return [
    {
      name: 'v2 mit nicht anwendbaren v3-Feldern',
      report: v2,
      title: 'Geprüfte Konfiguration in Ordnung',
      text: 'Schlüssel, Zertifikatskette und Sperrstatus erfolgreich geprüft.',
    },
    {
      name: 'vollständig bestätigt',
      report: happy,
      title: 'Geprüfte Konfiguration in Ordnung',
      text: 'Schlüssel, Zertifikatskette und Sperrstatus erfolgreich geprüft.',
    },
    {
      name: 'CRL ausgefallen',
      report: crlOffline,
      title: 'Prüfung mit offenen Punkten abgeschlossen',
      text: 'OCSP bestätigt ungesperrt; die CRL-Prüfung bleibt offen.',
    },
    {
      name: 'selbstsigniertes TEST-Zertifikat',
      report: testKey,
      title: 'Prüfung mit Hinweisen abgeschlossen',
      text: 'in TEST ist keine V-PKI-Verankerung erforderlich.',
    },
    {
      name: 'TEST ohne x5c',
      report: noChain,
      title: 'Prüfung mit Hinweisen abgeschlossen',
      text: 'Kein x5c veröffentlicht; in TEST zulässig.',
    },
    {
      name: 'gesperrt',
      report: revoked,
      title: 'Abweichungen gefunden',
      text: 'laut verifizierter Sperrauskunft gesperrt.',
    },
    {
      name: 'abgelaufen',
      report: expired,
      title: 'Abweichungen gefunden',
      text: 'außerhalb seiner Gültigkeitsdauer.',
    },
    {
      name: 'Online-Prüfung ausgeschaltet',
      report: offline,
      title: 'Prüfung mit offenen Punkten abgeschlossen',
      text: 'Online-Sperrprüfung ausgeschaltet',
    },
    {
      name: 'unvollständiger Bericht',
      report: partial,
      title: 'Prüfung mit offenen Punkten abgeschlossen',
      text: 'Zertifikatskette und Vertrauen nicht vollständig nachgewiesen.',
    },
    {
      name: 'einzelner Schlüssel',
      report: standalone,
      title: 'Geprüfte Schlüssel in Ordnung',
      text: 'Schlüssel, Zertifikatskette und Sperrstatus erfolgreich geprüft.',
    },
    {
      name: 'inaktiv',
      report: inactive,
      title: 'Abweichungen gefunden',
      text: 'Der Zustellpunkt ist inaktiv.',
    },
  ];
}
