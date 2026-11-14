use super::NetworkError;
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    net::{Ipv4Addr, Ipv6Addr},
    time::Duration,
};

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "mode", rename_all = "camelCase", deny_unknown_fields)]
pub enum ProxyMode {
    System,
    Explicit { server: String },
}

impl<'de> Deserialize<'de> for ProxyMode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Serde's internally tagged unit variants can discard extra fields.
        // An empty struct variant keeps the System contract strict too.
        #[derive(Deserialize)]
        #[serde(tag = "mode", rename_all = "camelCase", deny_unknown_fields)]
        enum Input {
            System {},
            Explicit { server: String },
        }
        Ok(match Input::deserialize(deserializer)? {
            Input::System {} => Self::System,
            Input::Explicit { server } => Self::Explicit { server },
        })
    }
}

impl fmt::Debug for ProxyMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::System => f.write_str("System"),
            Self::Explicit { .. } => f.write_str("Explicit { server: <redacted> }"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "NetworkOptions")]
pub struct NetworkConfig {
    proxy: ProxyMode,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NetworkOptions {
    proxy: ProxyMode,
}

impl TryFrom<NetworkOptions> for NetworkConfig {
    type Error = NetworkError;
    fn try_from(options: NetworkOptions) -> Result<Self, Self::Error> {
        Self::new(options.proxy)
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            proxy: ProxyMode::System,
        }
    }
}

impl NetworkConfig {
    pub fn new(proxy: ProxyMode) -> Result<Self, NetworkError> {
        if let ProxyMode::Explicit { server } = &proxy {
            validate_proxy(server)?;
        }
        Ok(Self { proxy })
    }

    pub fn proxy(&self) -> &ProxyMode {
        &self.proxy
    }
}

fn validate_proxy(server: &str) -> Result<(), NetworkError> {
    let valid = || {
        if server.len() > 320 || !server.is_ascii() {
            return None;
        }
        let (host, port) = if server.starts_with('[') {
            let (host, port) = server.split_once("]:")?;
            host.strip_prefix('[')?.parse::<Ipv6Addr>().ok()?;
            (host, port)
        } else {
            let (host, port) = server.rsplit_once(':')?;
            let is_numeric = host
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'.');
            if !(host.parse::<Ipv4Addr>().is_ok() || (!is_numeric && valid_dns_name(host))) {
                return None;
            }
            (host, port)
        };
        if host.is_empty() || port.starts_with('0') || !port.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        port.parse::<u16>().ok().filter(|port| *port > 0)
    };
    valid()
        .map(|_| ())
        .ok_or(NetworkError::InvalidConfiguration)
}

fn valid_dns_name(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.is_ascii()
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.as_bytes()[0].is_ascii_alphanumeric()
                && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

#[derive(Clone, PartialEq, Eq)]
pub struct HttpsUrl {
    host: String,
    request_target: String,
}

impl fmt::Debug for HttpsUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HttpsUrl(<redacted>)")
    }
}

impl HttpsUrl {
    /// Deliberately accepts a narrow HTTPS subset: DNS host, port 443 and an
    /// already percent-encoded ASCII path/query. Userinfo and fragments are invalid.
    pub fn parse(input: &str) -> Result<Self, NetworkError> {
        if input.len() > 32_768 || !valid_uri_bytes(input) {
            return Err(NetworkError::InvalidUrl);
        }
        let rest = input
            .strip_prefix("https://")
            .ok_or(NetworkError::InvalidUrl)?;
        let boundary = rest.find(['/', '?']).unwrap_or(rest.len());
        let authority = &rest[..boundary];
        let host = authority.strip_suffix(":443").unwrap_or(authority);
        if !valid_dns_name(host)
            || !host.contains('.')
            || host.parse::<Ipv4Addr>().is_ok()
            || host
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'.')
        {
            return Err(NetworkError::InvalidUrl);
        }
        let suffix = &rest[boundary..];
        let request_target = if suffix.starts_with('/') {
            suffix.into()
        } else {
            format!("/{suffix}")
        };
        Ok(Self {
            host: host.to_ascii_lowercase(),
            request_target,
        })
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn request_target(&self) -> &str {
        &self.request_target
    }
}

fn valid_uri_bytes(input: &str) -> bool {
    let bytes = input.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        if !(byte.is_ascii_alphanumeric() || b"-._~:/?@!$&'()*+,;=%".contains(&byte)) {
            return false;
        }
        if byte == b'%' {
            if at + 2 >= bytes.len()
                || !bytes[at + 1].is_ascii_hexdigit()
                || !bytes[at + 2].is_ascii_hexdigit()
            {
                return false;
            }
            at += 2;
        }
        at += 1;
    }
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowedHosts {
    hosts: Vec<String>,
}

impl AllowedHosts {
    /// Native/administrative source policy. Do not derive this allowlist from a
    /// redirect, downloaded document or renderer request.
    pub fn new(hosts: impl IntoIterator<Item = impl AsRef<str>>) -> Result<Self, NetworkError> {
        let mut result = Vec::new();
        for host in hosts {
            let host = host.as_ref().to_ascii_lowercase();
            if result.len() == 32
                || !valid_dns_name(&host)
                || !host.contains('.')
                || result.contains(&host)
            {
                return Err(NetworkError::InvalidConfiguration);
            }
            result.push(host);
        }
        if result.is_empty() {
            return Err(NetworkError::InvalidConfiguration);
        }
        Ok(Self { hosts: result })
    }

    pub fn permits(&self, url: &HttpsUrl) -> bool {
        self.hosts.contains(&url.host)
    }

    /// Absolute HTTPS and origin-relative redirects only. Scheme-relative and
    /// path-relative locations are rejected rather than interpreted ambiguously.
    pub fn redirect(
        &self,
        current: &HttpsUrl,
        location: &str,
        redirects_followed: u8,
    ) -> Result<HttpsUrl, NetworkError> {
        if redirects_followed >= 5 {
            return Err(NetworkError::RedirectLimit);
        }
        let next = if location.starts_with('/') && !location.starts_with("//") {
            HttpsUrl::parse(&format!("https://{}{location}", current.host))
        } else {
            HttpsUrl::parse(location)
        }
        .map_err(|_| NetworkError::RedirectRejected)?;
        if !self.permits(&next) {
            return Err(NetworkError::SourceNotAllowed);
        }
        Ok(next)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadLimits {
    pub(super) max_bytes: u64,
    pub(super) timeout: Duration,
}

impl DownloadLimits {
    pub fn new(max_bytes: u64, timeout: Duration) -> Result<Self, NetworkError> {
        if !(1..=16 * 1024 * 1024 * 1024).contains(&max_bytes)
            || timeout < Duration::from_secs(1)
            || timeout > Duration::from_secs(3600)
        {
            return Err(NetworkError::InvalidConfiguration);
        }
        Ok(Self { max_bytes, timeout })
    }
}
