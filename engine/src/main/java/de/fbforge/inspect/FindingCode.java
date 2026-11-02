package de.fbforge.inspect;

/** Machine-readable finding categories. Never derive these from translated titles. */
public enum FindingCode {
  GENERAL,
  DESTINATION_ID,
  DESTINATION_STATUS,
  TRUST_ANCHOR_MISSING,
  REVOCATION_NOT_REQUESTED,
  SCHEMA_REFERENCES_UNRESOLVED,
  PVOG_INPUT_REQUIRED
}
