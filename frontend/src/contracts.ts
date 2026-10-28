export type Status = 'confirmed' | 'error' | 'warning' | 'unknown' | 'notApplicable';
export interface Finding {
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
  data: Record<string, unknown>;
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
    id: 'help',
    label: 'Hilfe',
    icon: 'help',
    description: 'Abläufe, Quellen und Grenzen der Diagnose.',
  },
  { id: 'about', label: 'Info', icon: 'info', description: 'FBForge Inspect 0.1.0 · EUPL-1.2' },
] as const;
export type Page = (typeof pages)[number]['id'];
