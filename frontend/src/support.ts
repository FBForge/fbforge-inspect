import type { Report } from './contracts.ts';
import type { PortableResult } from './portable';
import { serverNetworkEvidence } from './serverEvidence.ts';
import { records } from './resultValues.ts';
export type SupportAudience = 'it' | 'operator' | 'editorial' | 'fitko';
const names: Record<SupportAudience, string> = {
  it: 'Lokale IT',
  operator: 'Zustellpunktbetreiber',
  editorial: 'Datenpflege',
  fitko: 'FITKO-Support',
};
export function supportText(
  report: Report,
  audience: SupportAudience,
  includeSources: boolean,
  runtime?: PortableResult['runtime'],
): string {
  const findings = report.findings.filter(
    (f) => f.status === 'error' || f.status === 'unknown' || f.status === 'warning',
  );
  const lines = [
    `Diagnose für ${names[audience]}`,
    `Umgebung: ${report.environment} · API: ${report.apiVersion}`,
    `Prüfzeit: ${report.createdAt} · Inspect ${report.appVersion}`,
    `Aufgabe: ${report.operation}`,
    '',
    'Erwartetes Verhalten: [bitte ergänzen]',
    'Beobachtetes Verhalten:',
  ];
  if (runtime)
    lines.splice(
      4,
      0,
      `Übermittelter Laufzeitkontext: ${runtime.os} · ${runtime.architecture} · Java ${runtime.javaVersion}`,
    );
  if (!findings.length)
    lines.push('In den geprüften Einzelpunkten wurden keine offenen Befunde festgestellt.');
  for (const finding of findings) {
    lines.push(`- ${finding.title}: ${finding.detail}`);
    if (finding.nextStep) lines.push(`  Nächster Schritt: ${finding.nextStep}`);
    if (includeSources)
      lines.push(`  Feldpfad: ${finding.path}`, `  Quelle: ${finding.source || 'Lokale Eingabe'}`);
  }
  if (includeSources && audience === 'it') {
    lines.push('', 'Öffentliche Abrufe:');
    for (const request of report.requests)
      lines.push(`${request.method} ${request.url} · ${request.status || request.outcome}`);
    const evidence = serverNetworkEvidence(report);
    const context = records([evidence.data.networkContext])[0];
    if (context?.httpsTrust) lines.push(`HTTPS-Vertrauen: ${context.httpsTrust}`);
    for (const trace of records(evidence.data.networkTraces)) {
      lines.push(`Verbindungsphasen: ${trace.url}`);
      for (const phase of records(trace.phases))
        lines.push(
          `  ${phase.stage}: ${phase.outcome} · ${phase.durationMs ?? 'Keine Messung'} ms`,
        );
    }
  }
  lines.push(
    '',
    'Die vorgeschlagene Ansprechstelle ist keine nachgewiesene Fehlerzuweisung.',
    'Die Diagnose betrifft öffentliche Einzelpunkte und ist keine authentifizierte Zustellprobe.',
  );
  return lines.join('\n');
}
export function explainHttp(code: number): string {
  if (code === 407)
    return 'Der Proxy verlangt eine Anmeldung. Die Antwort stammt vom Proxy. Prüfen Sie Verbindungsweg und unterstützte Anmeldeart.';
  if ([401, 403].includes(code))
    return 'Der Zugriff wurde zurückgewiesen. Prüfen Sie Umgebung und öffentliche Quelle. Daraus folgt kein leerer Suchbestand.';
  if (code === 404)
    return 'Unter dieser Adresse wurde kein Eintrag gefunden. Prüfen Sie Kennung, Umgebung und API-Pfad.';
  if (code === 429)
    return 'Das Ratenlimit ist erreicht. Beachten Sie vorhandene Retry-After-Angaben. Eine erneute Prüfung muss ausdrücklich gestartet werden.';
  if (code >= 500 && code <= 599)
    return 'Der antwortende Dienst oder eine vorgeschaltete Komponente meldet einen Serverfehler. Öffentliche Erreichbarkeit und offiziellen Betriebsstatus prüfen.';
  if (code >= 200 && code <= 299)
    return 'Der HTTP-Abruf war erfolgreich. Inhalt, Signaturen und Zustellbarkeit benötigen eigene Prüfungen.';
  return 'Dieser Status benötigt den Kontext der betroffenen Anfrage. Quelle, Methode und öffentliche Fehlerbeschreibung heranziehen.';
}
