import type { Report, Status } from './contracts.ts';
import { containsPrivateMaterial } from './inputs.ts';

export interface CheckInput {
  id: string;
  operation: string;
  input: Record<string, unknown>;
}
export interface PortableJob {
  format: 'fbforge-inspect-job';
  formatVersion: 1;
  environment: string;
  apiVersion: string;
  checks: CheckInput[];
  network?: { mode: 'direct' | 'manual'; proxyUrl?: string; httpsCaPem?: string };
}
export interface CheckResult {
  id: string;
  report?: Report;
  error?: string;
}
export interface PortableResult {
  format: 'fbforge-inspect-result';
  formatVersion: 1;
  appVersion: string;
  createdAt: string;
  jobSha256?: string;
  runtime: { os: string; architecture: string; javaVersion: string };
  checks: CheckResult[];
  exitCode: number;
}
const operations = new Set([
  'destination',
  'routing',
  'reverse',
  'network',
  'areas',
  'process',
  'limits',
  'keys',
  'certificate',
  'signature',
  'schema',
  'rollover',
]);
const states = new Set<Status>(['confirmed', 'error', 'warning', 'unknown', 'notApplicable']);
export function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value))
    throw new Error('Objekt erwartet.');
  return value as Record<string, unknown>;
}
function text(value: unknown, max = 65536): string {
  if (typeof value !== 'string' || value.length > max) throw new Error('Textfeld ungültig.');
  return value;
}
function parsed(source: string, limit: number) {
  if (new TextEncoder().encode(source).length > limit) throw new Error('Dateigröße überschritten.');
  const value: unknown = JSON.parse(source);
  function depth(v: unknown, level: number) {
    if (level > 48) throw new Error('Datei ist zu tief verschachtelt.');
    if (v && typeof v === 'object') for (const child of Object.values(v)) depth(child, level + 1);
  }
  depth(value, 0);
  if (containsPrivateMaterial(source)) throw new Error('Private Schlüssel sind nicht zulässig.');
  return object(value);
}
export function parseJob(source: string): PortableJob {
  const job = parsed(source, 2 * 1024 * 1024);
  if (
    Object.keys(job).some(
      (key) =>
        !['format', 'formatVersion', 'environment', 'apiVersion', 'checks', 'network'].includes(
          key,
        ),
    )
  )
    throw new Error('Unbekannte Felder im Prüfauftrag.');
  if (
    job.format !== 'fbforge-inspect-job' ||
    job.formatVersion !== 1 ||
    !['TEST', 'STAGE', 'PROD'].includes(String(job.environment)) ||
    !['v2', 'v3'].includes(String(job.apiVersion)) ||
    !Array.isArray(job.checks) ||
    !job.checks.length ||
    job.checks.length > 32
  )
    throw new Error('Prüfauftrag oder Formatversion ungültig.');
  const ids = new Set<string>();
  for (const item of job.checks) {
    const check = object(item);
    if (Object.keys(check).some((key) => !['id', 'operation', 'input'].includes(key)))
      throw new Error('Unbekannte Felder in der Prüfung.');
    const id = text(check.id, 64);
    if (!/^[A-Za-z0-9_-]+$/.test(id) || ids.has(id) || !operations.has(String(check.operation)))
      throw new Error('Prüfkennung oder Diagnosefunktion ungültig.');
    ids.add(id);
    const input = object(check.input);
    if (
      Object.keys(input).some(
        (key) =>
          ![
            'destinationId',
            'service',
            'region',
            'regionKind',
            'schema',
            'mime',
            'metadata',
            'compare',
            'revocation',
            'trustPem',
            'processModelId',
            'messageId',
            'fromDestinationId',
            'toDestinationId',
            'text',
            'url',
            'areaSearch',
            'pvog',
            'lbId',
            'pairs',
            'candidateKid',
            'certificates',
            'certificateEndpoints',
            'onlyUrl',
          ].includes(key),
      )
    )
      throw new Error('Unbekannte oder vertrauliche Prüfeingabe.');
    if (
      containsPrivateMaterial(String(input.text || ''), check.operation === 'keys') ||
      containsPrivateMaterial(String(input.trustPem || ''), true)
    )
      throw new Error('Private Schlüssel gehören nicht in Prüfaufträge.');
    if (input.pairs !== undefined) {
      if (!Array.isArray(input.pairs) || input.pairs.length > 12)
        throw new Error('Routing-Kombinationen ungültig.');
      for (const value of input.pairs)
        if (
          Object.keys(object(value)).some(
            (key) => !['service', 'region', 'regionKind'].includes(key),
          )
        )
          throw new Error('Unbekannte Routing-Kombination.');
    }
  }
  if (job.network) {
    const network = object(job.network);
    if (
      !['direct', 'manual'].includes(String(network.mode)) ||
      Object.keys(network).some((k) => !['mode', 'proxyUrl', 'httpsCaPem'].includes(k))
    )
      throw new Error('Zugangsdaten gehören nicht in den Prüfauftrag.');
    if (network.proxyUrl !== undefined) {
      const url = new URL(text(network.proxyUrl, 2048));
      if (
        !['http:', 'https:'].includes(url.protocol) ||
        url.username ||
        url.password ||
        url.search ||
        url.hash ||
        url.pathname !== '/'
      )
        throw new Error('Proxy-Adresse ungültig.');
    }
    if (network.mode === 'manual' && !network.proxyUrl) throw new Error('Proxy-Adresse fehlt.');
    if (network.httpsCaPem !== undefined) text(network.httpsCaPem, 256 * 1024);
  }
  return job as unknown as PortableJob;
}
/** Imported results remain untrusted evidence. Only the safe, textual viewer consumes them. */
export function parseResult(source: string): PortableResult {
  const result = parsed(source, 16 * 1024 * 1024);
  if (
    result.format !== 'fbforge-inspect-result' ||
    result.formatVersion !== 1 ||
    !Array.isArray(result.checks) ||
    result.checks.length > 32 ||
    !Number.isInteger(result.exitCode) ||
    ![0, 2, 3].includes(Number(result.exitCode))
  )
    throw new Error('Ergebnisdatei oder Formatversion ungültig.');
  text(result.appVersion, 64);
  if (!Number.isFinite(Date.parse(text(result.createdAt, 64))))
    throw new Error('Prüfzeit ungültig.');
  const runtime = object(result.runtime);
  for (const key of ['os', 'architecture', 'javaVersion']) text(runtime[key], 256);
  if (result.jobSha256 !== undefined && !/^[a-f0-9]{64}$/.test(String(result.jobSha256)))
    throw new Error('Auftragsfingerabdruck ungültig.');
  const ids = new Set<string>();
  for (const item of result.checks) {
    const entry = object(item);
    const id = text(entry.id, 64);
    if (!/^[A-Za-z0-9_-]+$/.test(id) || ids.has(id)) throw new Error('Prüfkennung ungültig.');
    ids.add(id);
    if (entry.error !== undefined) text(entry.error, 256);
    if (entry.report) {
      const report = object(entry.report);
      if (
        report.protocolVersion !== 1 ||
        !operations.has(String(report.operation)) ||
        !['TEST', 'STAGE', 'PROD'].includes(String(report.environment)) ||
        !['v2', 'v3'].includes(String(report.apiVersion)) ||
        !Array.isArray(report.findings) ||
        report.findings.length > 5000 ||
        !Array.isArray(report.requests) ||
        report.requests.length > 100
      )
        throw new Error('Diagnosebericht ungültig.');
      text(report.createdAt, 64);
      text(report.appVersion, 64);
      object(report.data);
      for (const value of report.findings) {
        const finding = object(value);
        if (!states.has(finding.status as Status)) throw new Error('Befundstatus ungültig.');
        for (const key of ['title', 'detail', 'path', 'source', 'nextStep', 'limitation'])
          text(finding[key]);
      }
      for (const value of report.requests) {
        const request = object(value);
        for (const key of ['url', 'method', 'at', 'outcome']) text(request[key]);
        for (const key of ['status', 'bytes'])
          if (!Number.isSafeInteger(request[key]) || Number(request[key]) < 0)
            throw new Error('Abfragebeleg ungültig.');
      }
      // Raw responses are unnecessary for server diagnosis and never retained on import.
      delete report.rawResponses;
    } else if (!entry.error) throw new Error('Ergebnis oder Fehlercode fehlt.');
  }
  return result as unknown as PortableResult;
}

export interface PasteContext {
  operation: 'destination' | 'routing';
  environment?: string;
  apiVersion?: string;
  input: Record<string, string>;
}
export function recognizeInput(source: string): PasteContext | null {
  const value = source.trim();
  const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
  if (uuid.test(value)) return { operation: 'destination', input: { destinationId: value } };
  try {
    const url = new URL(value);
    const hosts: Record<string, string> = {
      'test.fit-connect.fitko.dev': 'TEST',
      'stage.fit-connect.fitko.net': 'STAGE',
      'prod.fit-connect.fitko.net': 'PROD',
      'routing-api-testing.fit-connect.fitko.dev': 'TEST',
      'routing-api-prod.fit-connect.fitko.net': 'PROD',
    };
    if (
      url.protocol !== 'https:' ||
      !hosts[url.hostname] ||
      url.username ||
      url.password ||
      url.port ||
      url.hash
    )
      return null;
    const match = url.pathname.match(/^\/submission-api\/(v[23])\/destinations\/([^/]+)\/?$/);
    if (match && uuid.test(match[2]!) && !url.search)
      return {
        operation: 'destination',
        environment: hosts[url.hostname],
        apiVersion: match[1],
        input: { destinationId: match[2]! },
      };
    const service = url.searchParams.get('leikaKey') || '';
    const kinds = ['ars', 'ags', 'areaId'].filter((k) => url.searchParams.has(k));
    if (
      /^\/v2\/routes\/?$/.test(url.pathname) &&
      /^\d{14}$/.test(service) &&
      kinds.length === 1 &&
      [...url.searchParams.keys()].every((k) =>
        ['leikaKey', 'ars', 'ags', 'areaId', 'limit', 'offset'].includes(k),
      )
    ) {
      const kind = kinds[0]!;
      const region = url.searchParams.get(kind)!;
      if (
        !(kind === 'ars' ? /^(DE)?\d{12}$/ : kind === 'ags' ? /^(DE)?\d{8}$/ : /^\d{1,20}$/).test(
          region,
        )
      )
        return null;
      return {
        operation: 'routing',
        environment: hosts[url.hostname],
        input: { service, region, regionKind: kind },
      };
    }
  } catch {
    /* Unrecognized paste remains plain input. */
  }
  return null;
}

export function checksFromText(source: string): CheckInput[] {
  const lines = source
    .split(/\r?\n/)
    .map((v) => v.trim())
    .filter(Boolean);
  if (!lines.length || lines.length > 32) throw new Error('Zwischen 1 und 32 Zeilen erwartet.');
  return lines.map((line, index) => {
    const context = recognizeInput(line);
    if (context)
      return { id: `check-${index + 1}`, operation: context.operation, input: context.input };
    const [operation, service, region, kind = 'ars'] = line.split(',').map((v) => v.trim());
    if (
      operation === 'routing' &&
      /^\d{14}$/.test(service || '') &&
      region &&
      ['ars', 'ags', 'areaId'].includes(kind)
    )
      return {
        id: `check-${index + 1}`,
        operation,
        input: { service, region, regionKind: kind, pvog: false },
      };
    if (line === 'network') return { id: `check-${index + 1}`, operation: 'network', input: {} };
    throw new Error(
      `Zeile ${index + 1}: Destination-ID, unterstützte URL, network oder routing,LeiKa,Gebiet,Gebietsart erwartet.`,
    );
  });
}

export function exitCode(checks: CheckResult[]): number {
  if (checks.some((c) => c.error || c.report?.findings.some((f) => f.status === 'unknown')))
    return 3;
  return checks.some((c) => c.report?.findings.some((f) => f.status === 'error')) ? 2 : 0;
}
