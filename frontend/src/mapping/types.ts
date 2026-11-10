export interface SchemaDocument {
  name: string;
  text: string;
}
export interface MappingField {
  key: string;
  path: string[];
  names: string[];
  namespaces: string[];
  label: string;
  description: string;
  identifiers: string[];
  type: string;
  required: boolean;
  repeated: boolean;
  attribute: boolean;
  namespace: string;
  warnings: string[];
}
export interface MappingSchema {
  format: 'xsd' | 'json' | 'xdf';
  name: string;
  root: string;
  namespace: string;
  documents: SchemaDocument[];
  fields: MappingField[];
  warnings: string[];
  roots: string[];
}
export interface WorkbookRow {
  number: number;
  cells: Record<string, string>;
}
export interface WorkbookSheet {
  name: string;
  rows: WorkbookRow[];
}
export interface RequirementColumns {
  id: string;
  label: string;
  path: string[];
  type: string;
  note: string;
  codelist: string;
}
export interface Requirement {
  id: string;
  label: string;
  path: string[];
  group: string;
  type: string;
  note: string;
  sheet: string;
  row: number;
  cells: string[];
  codelist: { value: string; target: string }[];
}
export interface Requirements {
  name: string;
  sheet: string;
  namespace: string;
  columns: RequirementColumns;
  fields: Requirement[];
  constants: { value: string; path: string[]; sheet: string; row: number }[];
  warnings: string[];
}
export interface Candidate {
  target: string;
  score: number;
  reasons: string[];
  warnings: string[];
  evidence: Requirement[];
}
export type DecisionState = 'automatic' | 'review' | 'confirmed' | 'skipped';
export interface MappingDecision {
  source: string;
  target: string;
  state: DecisionState;
  candidates: Candidate[];
  reasons: string[];
  warnings: string[];
}
