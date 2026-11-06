/** Known report fields. Uninterpreted source fields remain available as evidence. */
export interface SubmissionSchema {
  schemaUri?: string;
  mimeType?: string;
}
export interface PublicService {
  identifier?: string;
  regions?: string[];
  submissionSchemas?: SubmissionSchema[];
}
export interface ProcessMessage {
  processModelId?: string;
  messageId?: string;
  schemas?: SubmissionSchema[];
}
export interface DestinationData {
  encryptionKid?: string;
  destinationId?: string;
  status?: string;
  type?: string;
  metadataVersions?: string[];
  publicServices?: PublicService[];
  processMessages?: ProcessMessage[];
}
export interface ContextRow {
  destinationId?: string;
  destinationName?: string;
  destinationShortName?: string;
  name?: string;
  id?: string;
  origin?: string;
  status?: string;
  type?: string;
}
export interface PvogCandidate {
  id: string;
  name?: string;
  source?: string;
  ars?: string[];
  organisationIds?: string[];
}
export interface CodeValue {
  code?: string;
  name?: string;
}
export interface PvogRelation {
  id?: unknown;
  rolle?: CodeValue | CodeValue[];
  leistungID?: unknown[];
  gebietID?: unknown[];
}
export interface PvogParent {
  uebergeordnetesObjektID?: unknown;
  uebergeordnetesObjektKlasse?: CodeValue;
  zustaendigkeit?: PvogRelation[];
}
export interface AddressingSignature {
  context?: string;
  verified?: boolean;
  contextMatches?: boolean;
  claims?: { destinationId?: string };
}
export interface OnlineParameter {
  parameterEfA?: CodeValue;
  parameterName?: string;
  parameterWert?: unknown;
  languageCode?: string[];
}
export interface OnlineService {
  bezeichnung?: { languageCode?: string; value?: string }[];
  parameterEfA?: OnlineParameter[];
  parameterIndividuell?: OnlineParameter[];
}
export interface OnlineServiceReference {
  id: string;
  sourceIndex: number;
  matchesParent?: boolean;
  source?: string;
}
export interface OnlineServiceSource {
  data?: OnlineService;
}
export interface PublicKeyData {
  kid?: string;
  algorithm?: string;
  bits?: number;
  x5cCount?: number;
  publicJwk?: Record<string, unknown>;
}
export interface CertificateData {
  signatureAlgorithm?: string;
  signatureAlgorithmOid?: string;
  subject?: string;
  role?: string;
  chainIndex?: number;
  kid?: string;
  keyPath?: string;
  issuer?: string;
  notAfter?: string;
  sha256?: string;
  notBefore?: string;
  pem?: string;
  x5c?: string;
}
export interface SchemaRow {
  path: string;
  name?: string;
  title?: string;
  type?: string | string[];
  base?: string;
  description?: string;
}
export interface SchemaReference {
  target: string;
  path?: string;
}
export interface ServiceOffer {
  serviceKey: string;
  identifier?: string;
  kind: string;
  schemas: SubmissionSchema[];
  regions?: string[];
  totalSchemas?: number;
}
export interface ReportData extends Record<string, unknown> {
  destination?: DestinationData;
  routes?: ContextRow[];
  destinations?: ContextRow[];
  areas?: ContextRow[];
  pvogCandidates?: PvogCandidate[];
  pvogRelations?: PvogParent[];
  addressingSignatures?: AddressingSignature[];
  pvogOnlineServices?: OnlineServiceReference[];
  pvogOnlineServiceSources?: OnlineServiceSource[];
  keys?: PublicKeyData[];
  certificates?: CertificateData[];
  structure?: SchemaRow[];
  references?: SchemaReference[];
  schemaSource?: string;
  dialect?: string;
  schemaVersion?: string;
  targetNamespace?: string;
  limits?: { attachments?: Record<string, Record<string, number>> };
}
