import type { ReportData } from './reportData';

export type Status = 'confirmed' | 'error' | 'warning' | 'unknown' | 'notApplicable';
export type FindingCode =
  | 'GENERAL'
  | 'DESTINATION_ID'
  | 'DESTINATION_STATUS'
  | 'DESTINATION_CONFIGURATION'
  | 'JWK_REQUIREMENTS'
  | 'KEY_DATA_INVALID'
  | 'CERTIFICATE_CHAIN'
  | 'CERTIFICATE_BINDING'
  | 'CERTIFICATE_VALIDITY'
  | 'CERTIFICATE_SIGNATURE'
  | 'CERTIFICATE_TRUST'
  | 'OCSP_STATUS'
  | 'CRL_STATUS'
  | 'REVOCATION_UNAVAILABLE'
  | 'TRUST_ANCHOR_MISSING'
  | 'REVOCATION_NOT_REQUESTED'
  | 'SCHEMA_REFERENCES_UNRESOLVED'
  | 'PVOG_INPUT_REQUIRED';
export interface Finding {
  /** Optional when reading older protocol-v1 fixtures and reports. */
  code?: FindingCode;
  status: Status;
  title: string;
  detail: string;
  path: string;
  source: string;
  nextStep: string;
  limitation: string;
}
export interface RequestEvidence {
  url: string;
  method: string;
  at: string;
  status: number;
  bytes: number;
  outcome: string;
  durationMs?: number;
  viaProxy?: boolean;
}
export interface Report {
  protocolVersion: number;
  appVersion: string;
  createdAt: string;
  operation: string;
  environment: string;
  apiVersion: string;
  findings: Finding[];
  requests: RequestEvidence[];
  data: ReportData;
  rawResponses?: Record<string, string>;
}
export interface EngineResponse {
  ok: boolean;
  report?: Report;
  error?: string;
}
export const labels: Record<Status, string> = {
  confirmed: 'Bestätigt',
  error: 'Fehler',
  warning: 'Hinweis',
  unknown: 'Nicht nachgewiesen',
  notApplicable: 'Nicht anwendbar',
};
export const pages = [
  {
    id: 'start',
    label: 'Start',
    icon: 'home',
    description: 'Öffentliche Schnittstellen. Klare Befunde.',
  },
  {
    id: 'destination',
    label: 'Destination prüfen',
    icon: 'target',
    description: 'Veröffentlichte Konfiguration, Leistungen und Schlüssel prüfen.',
  },
  {
    id: 'routing',
    label: 'Routing prüfen',
    icon: 'route',
    description: 'Zuständige Destinationen und öffentliche Quellen nachvollziehen.',
  },
  {
    id: 'process',
    label: 'Prozessnachricht prüfen',
    icon: 'process',
    description: 'Zustellpunkte über den öffentlichen v3-Prozessbezug finden.',
  },
  {
    id: 'keys',
    label: 'Schlüssel prüfen',
    icon: 'key',
    description: 'Öffentliche JWK, Zertifikate und Sperrstatus untersuchen.',
  },
  {
    id: 'schema',
    label: 'Fachdatenschemata',
    icon: 'schema',
    description: 'JSON Schema und XSD strukturiert lesen.',
  },
  {
    id: 'batch',
    label: 'Sammelprüfung',
    icon: 'process',
    description: 'Mehrere öffentliche Prüfungen vorbereiten und gezielt ausführen.',
  },
  {
    id: 'tools',
    label: 'Werkzeuge',
    icon: 'key',
    description: 'Rückwärtsrouting, Signaturen und Zertifikate gezielt analysieren.',
  },
  {
    id: 'inventory',
    label: 'Zertifikatsinventar',
    icon: 'key',
    description: 'Gültigkeit und betroffene Zustellpunkte überblicken.',
  },
  {
    id: 'server',
    label: 'Serverdiagnose',
    icon: 'network',
    description: 'Auf Windows und Linux prüfen und Ergebnisse hier auswerten.',
  },
  {
    id: 'network',
    label: 'Netzwerk',
    icon: 'network',
    description: 'Proxy einrichten und öffentliche Verbindungen prüfen.',
  },
  {
    id: 'help',
    label: 'Hilfe',
    icon: 'help',
    description: 'Abläufe, Quellen und Grenzen der Diagnose.',
  },
  { id: 'about', label: 'Info', icon: 'info', description: 'FBForge Inspect 0.2.0 · EUPL-1.2' },
] as const;
export type Page = (typeof pages)[number]['id'];
