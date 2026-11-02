import type { FindingCode, Report, Status } from './contracts.ts';

export interface SummaryItem {
  id: 'destination' | 'keys' | 'certificates' | 'revocation';
  label: string;
  status: Status;
  text: string;
}
export interface DiagnosisSummary {
  title: string;
  message: string;
  items: SummaryItem[];
}
const certificateCodes: FindingCode[] = [
  'CERTIFICATE_CHAIN',
  'CERTIFICATE_BINDING',
  'CERTIFICATE_VALIDITY',
  'CERTIFICATE_SIGNATURE',
  'CERTIFICATE_TRUST',
  'TRUST_ANCHOR_MISSING',
];
function checks(report: Report, codes: FindingCode[]) {
  return report.findings.filter((finding) => codes.includes(finding.code ?? 'GENERAL'));
}
function state(findings: { status: Status }[], fallback: Status = 'unknown'): Status {
  for (const status of ['error', 'unknown', 'warning', 'confirmed'] as const)
    if (findings.some((finding) => finding.status === status)) return status;
  return findings.length ? 'notApplicable' : fallback;
}
function confirmed(report: Report, code: FindingCode, minimum = 1) {
  const selected = checks(report, [code]);
  return (
    selected.length >= Math.max(1, minimum) &&
    selected.every((finding) => finding.status === 'confirmed')
  );
}
function destination(report: Report): SummaryItem {
  const selected = checks(report, [
    'DESTINATION_ID',
    'DESTINATION_STATUS',
    'DESTINATION_CONFIGURATION',
  ]);
  let status = state(selected);
  if (
    status === 'confirmed' &&
    (!report.data.destination ||
      report.data.destination.status !== 'active' ||
      !['DESTINATION_ID', 'DESTINATION_STATUS'].every((code) =>
        confirmed(report, code as FindingCode),
      ) ||
      state(checks(report, ['DESTINATION_CONFIGURATION'])) !== 'confirmed')
  )
    status = 'unknown';
  const text =
    status === 'confirmed'
      ? 'Aktiv; Kennung und veröffentlichte Konfiguration bestätigt.'
      : status === 'error'
        ? report.data.destination?.status === 'inactive'
          ? 'Der Zustellpunkt ist inaktiv.'
          : 'Kennung, Status oder veröffentlichte Konfiguration enthalten Abweichungen.'
        : status === 'warning'
          ? 'Hinweise zur veröffentlichten Konfiguration beachten.'
          : 'Öffentliche Konfiguration nicht vollständig bestätigt.';
  return { id: 'destination', label: 'Zustellpunkt', status, text };
}
function keys(report: Report): SummaryItem {
  const selected = checks(report, ['JWK_REQUIREMENTS', 'KEY_DATA_INVALID']);
  const count = report.data.keys?.length ?? 0;
  let status = state(selected);
  // Every supported RSA JWK has seven independent requirement findings. A partial report cannot imply success.
  if (status === 'confirmed' && (!count || !confirmed(report, 'JWK_REQUIREMENTS', count * 7)))
    status = 'unknown';
  const text =
    status === 'confirmed'
      ? count === 1
        ? 'Der öffentliche Schlüssel erfüllt die geprüften JWK-Vorgaben.'
        : 'Alle öffentlichen Schlüssel erfüllen die geprüften JWK-Vorgaben.'
      : status === 'notApplicable'
        ? 'Für diesen Zustellpunkttyp ist kein Verschlüsselungsschlüssel erforderlich.'
        : status === 'error'
          ? checks(report, ['KEY_DATA_INVALID']).length
            ? 'Schlüssel- oder Zertifikatsdaten sind nicht vollständig auswertbar.'
            : 'Mindestens ein Schlüssel erfüllt die geprüften JWK-Vorgaben nicht.'
          : 'Öffentlicher Schlüssel nicht vollständig geprüft oder nicht verfügbar.';
  return { id: 'keys', label: 'Schlüssel', status, text };
}
function certificates(report: Report): SummaryItem {
  const selected = checks(report, certificateCodes);
  const count = report.data.certificates?.length ?? 0;
  const withChain = Math.max(
    (report.data.keys ?? []).filter((key) => (key.x5cCount ?? 0) > 0).length,
    count ? 1 : 0,
  );
  const binding = confirmed(report, 'CERTIFICATE_BINDING', withChain);
  const dates = confirmed(report, 'CERTIFICATE_VALIDITY', count);
  const trust = checks(report, ['CERTIFICATE_TRUST', 'TRUST_ANCHOR_MISSING']);
  const missing = checks(report, ['CERTIFICATE_CHAIN']);
  let status = state(selected);
  if (
    status !== 'error' &&
    !(missing.length && !count) &&
    (!count || !binding || !dates || trust.length < withChain)
  )
    status = 'unknown';
  let text = 'Zertifikatskette und Vertrauen nicht vollständig nachgewiesen.';
  if (!count && missing.length)
    text =
      report.environment === 'TEST' && status === 'warning'
        ? 'Kein x5c veröffentlicht; in TEST zulässig.'
        : 'Keine auswertbare Zertifikatskette vorhanden.';
  else if (status === 'error') {
    const failed = selected.filter((finding) => finding.status === 'error');
    text = failed.some((finding) => finding.code === 'CERTIFICATE_VALIDITY')
      ? 'Mindestens ein Zertifikat liegt außerhalb seiner Gültigkeitsdauer.'
      : failed.some((finding) => finding.code === 'CERTIFICATE_BINDING')
        ? 'JWK und Zertifikat passen nicht zusammen.'
        : failed.some((finding) => finding.code === 'CERTIFICATE_SIGNATURE')
          ? 'Mindestens eine Zertifikatssignatur ist ungültig.'
          : 'Keine gültige Kette zum geprüften Vertrauensanker.';
  } else if (status === 'confirmed') {
    const evidence = Array.isArray(report.data.certificateTrust)
      ? (report.data.certificateTrust as { trusted?: boolean; source?: string }[])
      : [];
    const vpki =
      evidence.length >= withChain &&
      evidence.every((entry) => entry.trusted === true && entry.source === 'bundled-vpki');
    text = vpki
      ? 'Zertifikate zeitlich gültig; Schlüsselbindung und V-PKI-Vertrauenskette bestätigt.'
      : 'Zertifikate zeitlich gültig; Schlüsselbindung und geprüfte Vertrauenskette bestätigt.';
  } else if (status === 'warning')
    text = missing.length
      ? 'Teilweise ohne x5c; in TEST zulässig.'
      : 'Laufzeiten und Schlüsselbindung bestätigt; in TEST ist keine V-PKI-Verankerung erforderlich.';
  else if (binding && dates)
    text = 'Laufzeiten und Schlüsselbindung bestätigt; Zertifikatsvertrauen noch offen.';
  return { id: 'certificates', label: 'Zertifikate', status, text };
}
function revocation(report: Report): SummaryItem {
  const ocsp = checks(report, ['OCSP_STATUS']),
    crl = checks(report, ['CRL_STATUS']);
  const unavailable = checks(report, ['REVOCATION_UNAVAILABLE', 'REVOCATION_NOT_REQUESTED']);
  const missingChain = checks(report, ['CERTIFICATE_CHAIN']);
  const selected = [...ocsp, ...crl, ...unavailable];
  let status = state(selected);
  const trust = Array.isArray(report.data.certificateTrust)
    ? (report.data.certificateTrust as { chain?: unknown[] }[])
    : [];
  const expected = Math.max(
    1,
    trust.reduce(
      (count, entry) =>
        count + (Array.isArray(entry?.chain) ? Math.max(0, entry.chain.length - 1) : 0),
      0,
    ),
    (report.data.certificates ?? []).filter(
      (certificate) =>
        certificate.subject && certificate.issuer && certificate.subject !== certificate.issuer,
    ).length,
  );
  if (
    status === 'confirmed' &&
    new Set([...ocsp, ...crl].map((finding) => finding.path).filter(Boolean)).size < expected
  )
    status = 'unknown';
  if (
    !selected.length &&
    missingChain.length &&
    report.environment === 'TEST' &&
    state(missingChain) === 'warning'
  )
    status = 'warning';
  const ocspGood = ocsp.length > 0 && state(ocsp) === 'confirmed';
  const crlGood = crl.length > 0 && state(crl) === 'confirmed';
  let text = 'Öffentlicher Sperrstatus nicht vollständig nachgewiesen.';
  if (status === 'confirmed')
    text =
      ocspGood && crlGood
        ? 'OCSP und CRL bestätigen: kein Sperrvermerk gefunden.'
        : ocspGood
          ? 'OCSP bestätigt: kein Sperrvermerk gefunden.'
          : 'CRL bestätigt: kein Sperrvermerk gefunden.';
  else if (status === 'error')
    text = 'Mindestens ein Zertifikat ist laut verifizierter Sperrauskunft gesperrt.';
  else if (status === 'warning')
    text = 'TEST-Ausnahme: keine vollständige öffentliche Sperrauskunft verfügbar.';
  else if (unavailable.some((finding) => finding.code === 'REVOCATION_NOT_REQUESTED'))
    text = 'Online-Sperrprüfung ausgeschaltet; kein aktueller Sperrstatus geprüft.';
  else if (ocspGood && crl.length)
    text = 'OCSP bestätigt ungesperrt; die CRL-Prüfung bleibt offen.';
  else if (crlGood && ocsp.length)
    text = 'CRL bestätigt ungesperrt; die OCSP-Prüfung bleibt offen.';
  return { id: 'revocation', label: 'Sperrstatus', status, text };
}

/** Conclusions use machine codes and complete evidence, never translated finding titles or just a green count. */
export function diagnosisSummary(report: Report): DiagnosisSummary | null {
  if (!report.data || !['destination', 'keys'].includes(report.operation)) return null;
  const items: SummaryItem[] = [];
  if (report.operation === 'destination') items.push(destination(report));
  const hasKeys =
    report.operation === 'keys' ||
    (report.data.keys?.length ?? 0) > 0 ||
    Boolean(report.data.destination?.encryptionKid) ||
    checks(report, ['JWK_REQUIREMENTS', 'KEY_DATA_INVALID']).length > 0;
  if (hasKeys) items.push(keys(report));
  const hasCertificates =
    (report.data.keys?.length ?? 0) > 0 ||
    (report.data.certificates?.length ?? 0) > 0 ||
    checks(report, certificateCodes).length > 0 ||
    (report.data.keys ?? []).some((key) => (key.x5cCount ?? 0) > 0);
  if (hasCertificates) items.push(certificates(report), revocation(report));
  if (!items.length) return null;
  const status = state([...report.findings, ...items]);
  const title =
    status === 'error'
      ? 'Abweichungen gefunden'
      : status === 'unknown'
        ? 'Prüfung mit offenen Punkten abgeschlossen'
        : status === 'warning'
          ? 'Prüfung mit Hinweisen abgeschlossen'
          : report.operation === 'destination'
            ? 'Geprüfte Konfiguration in Ordnung'
            : 'Geprüfte Schlüssel in Ordnung';
  const issue = items.find((item) => item.status === status);
  const completeCrypto = ['keys', 'certificates', 'revocation'].every((id) =>
    items.some((item) => item.id === id && item.status === 'confirmed'),
  );
  const message =
    status === 'confirmed' || status === 'notApplicable'
      ? completeCrypto
        ? `${report.operation === 'destination' ? 'Zustellpunkt aktiv. ' : ''}Schlüssel, Zertifikatskette und Sperrstatus erfolgreich geprüft.`
        : 'Die ausgewerteten öffentlichen Angaben sind für die geprüften Punkte stimmig.'
      : (issue?.text ??
        (status === 'error'
          ? 'Weitere Einzelbefunde enthalten Fehler oder Abweichungen.'
          : 'Weitere Einzelbefunde enthalten Hinweise oder noch offene Nachweise.'));
  return { title, message, items };
}
