import type { Report, Status } from './contracts.ts';
import type { CheckResult } from './portable.ts';

export const operationNames: Record<string, string> = {
  destination: 'Zustellpunkt',
  routing: 'Routing',
  network: 'Netzwerk',
  reverse: 'Rückwärtsrouting',
  keys: 'Öffentliche Schlüssel',
  certificate: 'Zertifikat',
  signature: 'Adressierungssignatur',
  rollover: 'Schlüsselwechsel',
  schema: 'Schemastruktur',
  process: 'Prozessbezug',
  limits: 'Limits',
  areas: 'Gebietssuche',
};
export function resultState(result?: CheckResult): Status | 'pending' {
  if (!result) return 'pending';
  if (result.error || !result.report) return 'unknown';
  const findings = result.report.findings;
  for (const status of ['error', 'unknown', 'warning', 'confirmed'] as const)
    if (findings.some((f) => f.status === status)) return status;
  return findings.length ? 'notApplicable' : 'unknown';
}
export const outcomeNames = {
  error: 'Handlungsbedarf',
  unknown: 'Nicht nachgewiesen',
  warning: 'Hinweise beachten',
  confirmed: 'Geprüfte Einzelpunkte bestätigt',
  notApplicable: 'Nicht anwendbar',
  pending: 'Noch nicht ausgeführt',
};
export function attention(report?: Report) {
  return (
    report?.findings.find((f) => f.status === 'error') ||
    report?.findings.find((f) => f.status === 'unknown') ||
    report?.findings.find((f) => f.status === 'warning')
  );
}
export async function jobDigest(bytes: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', new Uint8Array(bytes));
  return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, '0')).join('');
}
