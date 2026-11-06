import type { Report } from './contracts.ts';
import { object } from './portable.ts';
export interface InventoryUse {
  destination: string;
  environment: string;
  apiVersion: string;
  kid: string;
  active: boolean;
}
export interface InventoryCertificate {
  fingerprint: string;
  subject: string;
  issuer: string;
  notAfter: string;
  notBefore: string;
  uses: InventoryUse[];
  days: number;
  validity: 'valid' | 'expired' | 'future';
}
export function certificateInventory(reports: Report[], now = Date.now()): InventoryCertificate[] {
  const groups = new Map<string, InventoryCertificate>();
  for (const report of reports) {
    const data = report.data;
    const destination =
      data.destination && typeof data.destination === 'object' && !Array.isArray(data.destination)
        ? object(data.destination)
        : {};
    const certificates = Array.isArray(data.certificates) ? data.certificates : [];
    for (const value of certificates) {
      if (!value || typeof value !== 'object' || Array.isArray(value)) continue;
      const cert = object(value);
      if (
        cert.chainIndex !== 0 ||
        typeof cert.sha256 !== 'string' ||
        !/^[a-f0-9]{64}$/i.test(cert.sha256)
      )
        continue;
      if (
        typeof cert.notAfter !== 'string' ||
        typeof cert.notBefore !== 'string' ||
        !Number.isFinite(Date.parse(cert.notAfter)) ||
        !Number.isFinite(Date.parse(cert.notBefore))
      )
        continue;
      const fingerprint = cert.sha256.toLowerCase();
      let row = groups.get(fingerprint);
      if (!row) {
        row = {
          fingerprint,
          subject: String(cert.subject || ''),
          issuer: String(cert.issuer || ''),
          notAfter: cert.notAfter,
          notBefore: cert.notBefore,
          uses: [],
          days: Math.ceil((Date.parse(cert.notAfter) - now) / 86400000),
          validity:
            Date.parse(cert.notBefore) > now
              ? 'future'
              : Date.parse(cert.notAfter) < now
                ? 'expired'
                : 'valid',
        };
        groups.set(fingerprint, row);
      }
      const use = {
        destination:
          typeof destination.destinationId === 'string'
            ? destination.destinationId
            : 'Öffentliche Schlüsseleingabe',
        environment: report.environment,
        apiVersion: report.apiVersion,
        kid: typeof cert.kid === 'string' ? cert.kid : '',
        active:
          typeof destination.encryptionKid === 'string' && destination.encryptionKid === cert.kid,
      };
      if (
        !row.uses.some(
          (u) =>
            u.destination === use.destination &&
            u.environment === use.environment &&
            u.apiVersion === use.apiVersion &&
            u.kid === use.kid,
        )
      )
        row.uses.push(use);
    }
  }
  return [...groups.values()].sort((a, b) => Date.parse(a.notAfter) - Date.parse(b.notAfter));
}
export function renewalCalendar(rows: InventoryCertificate[], beforeDays: number): string {
  const lines = [
    'BEGIN:VCALENDAR',
    'VERSION:2.0',
    'PRODID:-//FBForge//Inspect 0.2.0//DE',
    'CALSCALE:GREGORIAN',
  ];
  for (const row of rows) {
    const due = new Date(Date.parse(row.notAfter) - beforeDays * 86400000)
      .toISOString()
      .slice(0, 10)
      .replaceAll('-', '');
    lines.push(
      'BEGIN:VEVENT',
      `UID:${row.fingerprint}-${beforeDays}@inspect.fbforge.de`,
      `DTSTAMP:${new Date()
        .toISOString()
        .replaceAll(/[-:]/g, '')
        .replace(/\.\d{3}Z$/, 'Z')}`,
      `DTSTART;VALUE=DATE:${due}`,
      'SUMMARY:FIT-Connect-Zertifikat erneuern',
      `DESCRIPTION:Fingerabdruck ${row.fingerprint}\\nBetroffene Zuordnungen ${row.uses.length}`,
      'END:VEVENT',
    );
  }
  lines.push('END:VCALENDAR', '');
  return lines.join('\r\n');
}
