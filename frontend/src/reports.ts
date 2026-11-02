import type { Report } from './contracts.ts';
import {
  overview,
  overviewDetail,
  findingDetail,
  findingNext,
  specificLimitation,
  statusOrder,
  statusLabel,
} from './diagnosis.ts';
export function reportForExport(report: Report, raw = false): Report {
  // Reports contain JSON values only. JSON cloning also accepts Vue's reactive proxies.
  if (raw) return JSON.parse(JSON.stringify(report));
  const { rawResponses: _excluded, ...included } = report;
  return JSON.parse(JSON.stringify(included));
}
export function escapeHtml(value: string): string {
  return value.replace(
    /[&<>"']/g,
    (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!,
  );
}
export function reportHtml(report: Report): string {
  const e = escapeHtml;
  const findings = [...report.findings].sort(
    (a, b) => statusOrder.indexOf(a.status) - statusOrder.indexOf(b.status),
  );
  const content = findings
    .map(
      (f) =>
        `<article><h3>${e(f.title)} <small>${e(statusLabel(f.status))}</small></h3><p>${e(findingDetail(f))}</p>${findingNext(f) ? `<p><strong>Nächster Schritt:</strong> ${e(findingNext(f))}</p>` : ''}${specificLimitation(f) ? `<p><strong>Beweisgrenze:</strong> ${e(specificLimitation(f))}</p>` : ''}<details><summary>Beleg und Feldpfad</summary><p>${e(f.path)}</p><p>${e(f.source || 'Lokale Eingabe')}</p></details></article>`,
    )
    .join('');
  return `<!doctype html><html lang="de"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'"><title>FBForge Inspect · Diagnosebericht</title><style>body{font:16px/1.5 system-ui;max-width:1000px;margin:24px auto;padding:0 24px;color:#1e3044;background:#fff}pre{white-space:pre-wrap;overflow-wrap:anywhere;background:#f2f5f8;padding:16px;border-radius:8px}h1{color:#005c8f}article{border-bottom:1px solid #cdd8e3;padding:12px 0}small{font-size:14px;color:#526579}p{overflow-wrap:anywhere}summary{cursor:pointer}</style><h1>FBForge Inspect</h1><p>Diagnosebericht · ${e(report.environment)} · ${e(report.apiVersion)} · ${e(report.createdAt)}</p><h2>${e(overview(report))}</h2><p>${e(overviewDetail(report))}</p>${content}<details><summary>Vollständige Berichtsdaten</summary><pre>${e(JSON.stringify(report, null, 2))}</pre></details></html>`;
}
