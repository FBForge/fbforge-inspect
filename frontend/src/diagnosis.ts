import { diagnosisSummary } from './diagnosisSummary.ts';
import { labels, type Finding, type Report, type Status } from './contracts.ts';

export const statusOrder: Status[] = ['error', 'warning', 'unknown', 'confirmed', 'notApplicable'];
export function overview(report: Report) {
  const summary = diagnosisSummary(report);
  if (summary) return summary.title;
  const count = (status: Status) => report.findings.filter((f) => f.status === status).length;
  if (count('error')) return 'Abweichungen gefunden';
  if (count('unknown')) return 'Prüfung mit offenen Punkten abgeschlossen';
  if (count('warning')) return 'Prüfung mit Hinweisen abgeschlossen';
  if (report.operation === 'schema') return 'Schema eingelesen';
  if (report.operation === 'limits') return 'Limits abgerufen';
  if (report.operation === 'network') return 'Verbindungen geprüft';
  return 'Einzelprüfungen abgeschlossen';
}
export function overviewDetail(report: Report) {
  if (report.operation === 'network')
    return 'Erreichbarkeit der geprüften öffentlichen Ziele und gewählter Verbindungsweg. Zertifikatsvertrauen und Sperrstatus werden separat geprüft.';
  if (report.operation === 'schema')
    return 'Syntax- und Strukturansicht, keine vollständige Schema- oder Instanzvalidierung.';
  if (report.operation === 'limits')
    return 'Öffentliche Größenlimits für das angegebene Paar. Keine Sendeprobe.';
  if (report.operation === 'areas')
    return 'Gebiete aus der gewählten öffentlichen Quelle. Ein passendes Gebiet kann für die Routingabfrage übernommen werden.';
  if (report.operation === 'routing' || report.operation === 'process')
    return 'Momentaufnahme öffentlicher Quellen. Fehlende oder ungeprüfte Nachweise belegen keine interne Routingursache.';
  return 'Die Bewertung gilt für die geprüften Einzelpunkte und ist keine Gesamtfreigabe des Zustellpunkts.';
}
export function findingDetail(f: Finding) {
  if (f.code === 'DESTINATION_STATUS')
    return f.detail === 'active'
      ? 'Der Zustellpunkt ist aktiv.'
      : f.detail === 'inactive'
        ? 'Der Zustellpunkt ist inaktiv.'
        : f.detail;
  return f.detail;
}
export function findingNext(f: Finding) {
  if (f.status === 'error' && f.code === 'DESTINATION_STATUS')
    return 'Status beim Betreiber des Zustellpunkts klären.';
  if (f.status === 'error' && f.code === 'DESTINATION_ID')
    return 'Destination-ID und Umgebung mit der angefragten Quelle abgleichen.';
  if (f.nextStep !== 'Quelle oder Konfiguration gezielt prüfen.') return f.nextStep;
  if (f.code === 'TRUST_ANCHOR_MISSING')
    return 'Den mitgelieferten V-PKI-Truststore und die Zertifikatskette prüfen. Zusätzliche CA-Zertifikate können unter Zertifikate und Sperrstatus geladen werden.';
  if (f.code === 'REVOCATION_NOT_REQUESTED')
    return 'Die ausgeschaltete OCSP-/CRL-Prüfung unter Zertifikate und Sperrstatus wieder aktivieren und erneut prüfen.';
  if (f.code === 'SCHEMA_REFERENCES_UNRESOLVED')
    return 'Eine Referenz bei Bedarf als eigene Prüfung vorbereiten. Das Ausgangsschema bleibt erhalten.';
  if (f.code === 'PVOG_INPUT_REQUIRED')
    return 'ARS als Gebietsart wählen und einen 12-stelligen ARS eingeben. Eine bekannte Leistungsbeschreibungs-ID kann direkt eingetragen oder als Vorschlag gesucht werden.';
  return '';
}
export function specificLimitation(f: Finding) {
  return f.limitation === 'Mit den verfügbaren öffentlichen Daten nicht nachgewiesen.'
    ? ''
    : f.limitation;
}
export function statusLabel(status: string) {
  return labels[status as Status] || status;
}
