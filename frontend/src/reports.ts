import type { Report } from './contracts.ts';
export function reportForExport(report: Report, raw = false): Report {
  // Reports contain JSON values only. JSON cloning also accepts Vue's reactive proxies.
  const copy: Report = JSON.parse(JSON.stringify(report));
  if (!raw) delete copy.rawResponses;
  return copy;
}
export function escapeHtml(value: string): string {
  return value.replace(
    /[&<>"']/g,
    (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!,
  );
}
export function reportHtml(report: Report): string {
  return `<!doctype html><html lang="de"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'"><title>FBForge Inspect · Diagnosebericht</title><style>body{font:16px system-ui;max-width:1000px;margin:40px auto;padding:0 24px;color:#1e3044}pre{white-space:pre-wrap;overflow-wrap:anywhere;background:#f2f5f8;padding:20px;border-radius:8px}h1{color:#005c8f}</style><h1>FBForge Inspect</h1><p>Diagnosebericht · ${escapeHtml(report.environment)} · ${escapeHtml(report.apiVersion)} · ${escapeHtml(report.createdAt)}</p><p>Momentaufnahme öffentlicher Quellen. Ein bestätigter Einzelbefund ist keine Freigabe des gesamten Zustellpunkts.</p><pre>${escapeHtml(JSON.stringify(report, null, 2))}</pre></html>`;
}
