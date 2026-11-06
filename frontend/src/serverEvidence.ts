import type { Report } from './contracts';
import { records } from './resultValues.ts';

const text = (value: unknown, limit = 8192) =>
  typeof value === 'string' ? value.slice(0, limit) : '';
const number = (value: unknown) =>
  typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : undefined;

function tlsEvidence(value: unknown) {
  const tls = records([value])[0];
  if (!tls) return undefined;
  return {
    protocol: text(tls.protocol, 64),
    cipher: text(tls.cipher, 128),
    certificates: records(tls.certificates)
      .slice(0, 10)
      .map((cert) => ({
        subject: text(cert.subject),
        issuer: text(cert.issuer),
        notAfter: text(cert.notAfter, 64),
        sha256: text(cert.sha256, 128),
      })),
  };
}

/** Imported data gets its own narrow presentation model before using the transport viewer. */
export function serverNetworkEvidence(report: Report): Report {
  const context = records([report.data.networkContext])[0];
  return {
    ...report,
    data: {
      networkContext: { httpsTrust: text(context?.httpsTrust, 1024) },
      networkChecks: records(report.data.networkChecks)
        .slice(0, 16)
        .map((item) => ({
          name: text(item.name, 256),
          host: text(item.host, 256),
          port: number(item.port),
          url: text(item.url),
          method: text(item.method, 16),
          detail: text(item.detail),
          status: text(item.status, 32),
          httpStatus: number(item.httpStatus),
          durationMs: number(item.durationMs),
          code: text(item.code, 128),
        })),
      networkTraces: records(report.data.networkTraces)
        .slice(0, 100)
        .map((trace) => {
          return {
            url: text(trace.url),
            route: text(trace.route, 32),
            phases: records(trace.phases)
              .slice(0, 16)
              .map((phase) => ({
                stage: text(phase.stage, 32),
                durationMs: number(phase.durationMs),
                outcome: text(phase.outcome, 128),
              })),
            tls: tlsEvidence(trace.tls),
            proxyTls: tlsEvidence(trace.proxyTls),
          };
        }),
    },
  };
}
