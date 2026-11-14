//! Operator-provisioned public trust configuration, changed through reviewed
//! application updates rather than imported from network responses.
use crate::{
    authenticode::{AuthenticodePolicies, AuthenticodePolicy, PolicyError, PolicyStatus},
    trust::{KeyStatus, MinisignTrustAnchor, TrustError, TrustStore},
};

/// Complete Tauri/Minisign public-key encoding supplied by the operator.
/// Its wrapper comment conveys no authenticated publisher identity. Update
/// signatures must separately authenticate their version and metadata bindings.
pub const SUITE_UPDATER_PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDc4OUE2RjA5Q0IzNDVCNkUKUldSdVd6VExDVythZUZtMk4yYVNPQmJJdkg4WXJKRVRSYnM5VDJhMER4dzRabCtrbmU0QXdxOEsK";

/// Public certificate selector used by the protected signing workflow.
/// This SHA1 thumbprint is deliberately separate from the native SHA256 pin.
pub const WINDOWS_SIGNING_THUMBPRINT: &str = "B3477D8A158AF7D599C2C17689CF541B1731D522";

const CERTIFICATE_SHA256: [u8; 32] = [
    0x55, 0x36, 0xdf, 0xf8, 0x3a, 0x31, 0x24, 0xcc, 0xb3, 0x54, 0x49, 0xd6, 0x27, 0xe8, 0xb9, 0x8d,
    0x01, 0xee, 0x42, 0x9b, 0xc2, 0x46, 0x5b, 0xe9, 0x49, 0x4b, 0x5a, 0x98, 0x06, 0xb4, 0xd7, 0x59,
];

pub fn bundled_authenticode_policies() -> Result<AuthenticodePolicies, PolicyError> {
    let products = [
        (
            "fbforge-connect-production",
            "fbforge-connect",
            "org.fitconnect.integration-client",
        ),
        (
            "fbforge-inspect-production",
            "fbforge-inspect",
            "de.fbforge.inspect",
        ),
        (
            "fbforge-suite-production",
            "fbforge-suite",
            "de.fbforge.suite",
        ),
    ];
    AuthenticodePolicies::new(
        products
            .into_iter()
            .map(|(id, tool_id, app_id)| AuthenticodePolicy {
                id: id.into(),
                tool_id: tool_id.into(),
                app_id: app_id.into(),
                certificate_sha256: vec![CERTIFICATE_SHA256],
                status: PolicyStatus::Active,
            })
            .collect(),
    )
}

/// Stable outer-envelope key ID for the operator-confirmed Minisign key.
/// This does not replace the embedded key-ID and signature checks.
pub const METADATA_KEY_ID: &str = "fbforge-metadata-789a6f09cb345b6e";

/// Provisioned metadata authority, never extended by downloaded catalogs.
/// No raw-v1 anchors or Suite self-update subject are implicitly authorized.
pub fn bundled_metadata_trust_store() -> Result<TrustStore, TrustError> {
    TrustStore::new(vec![])?.with_minisign_anchors(vec![MinisignTrustAnchor {
        id: METADATA_KEY_ID.into(),
        public_key: SUITE_UPDATER_PUBLIC_KEY.into(),
        subjects: vec![
            "catalog".into(),
            "release/fbforge-connect".into(),
            "release/fbforge-inspect".into(),
        ],
        // Explicit code policy: no additional local calendar restriction.
        // Signed metadata still expires within at most 30 days.
        not_before: 0,
        expires_at: u64::MAX,
        status: KeyStatus::Active,
    }])
}
