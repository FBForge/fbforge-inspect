//! Native HTTPS transport. A successful transfer proves neither package origin
//! nor metadata freshness. Callers stage privately and discard partial output
//! on every error before applying signature and exact package checks.
mod policy;
#[cfg(windows)]
mod windows;

pub use policy::{AllowedHosts, DownloadLimits, HttpsUrl, NetworkConfig, ProxyMode};
use std::{
    fmt,
    io::Write,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkError {
    InvalidConfiguration,
    InvalidUrl,
    SourceNotAllowed,
    RedirectRejected,
    RedirectLimit,
    Cancelled,
    Timeout,
    Offline,
    ProxyUnavailable,
    ProxyAuthenticationRequired,
    AuthenticationRequired,
    TlsRejected,
    TlsValidationUnavailable,
    HttpStatus(u16),
    ResponseTooLarge,
    InvalidResponse,
    DestinationIo,
    UnsupportedPlatform,
    Unavailable,
}

impl NetworkError {
    pub fn is_transient(self) -> bool {
        matches!(
            self,
            Self::Timeout
                | Self::Offline
                | Self::ProxyUnavailable
                | Self::HttpStatus(429 | 500 | 502 | 503 | 504)
        )
    }
}
impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidConfiguration => "Die Netzwerkeinstellungen sind ungültig.",
            Self::InvalidUrl => "Die Downloadadresse wird nicht unterstützt.",
            Self::SourceNotAllowed => "Diese Downloadquelle ist nicht freigegeben.",
            Self::RedirectRejected | Self::RedirectLimit => {
                "Die Weiterleitung wurde aus Sicherheitsgründen beendet."
            }
            Self::Cancelled => "Der Download wurde abgebrochen.",
            Self::Timeout => "Die Netzwerkverbindung hat das Zeitlimit überschritten.",
            Self::Offline => "Die Netzwerkverbindung ist nicht verfügbar.",
            Self::ProxyUnavailable => "Der eingestellte Proxy ist nicht erreichbar.",
            Self::ProxyAuthenticationRequired => {
                "Der Proxy benötigt eine Anmeldung. Wenden Sie sich an Ihre Administration."
            }
            Self::AuthenticationRequired => "Die Downloadquelle benötigt eine Anmeldung.",
            Self::TlsRejected => {
                "Die sichere Verbindung wurde abgewiesen. Prüfen Sie die Zertifikatsvorgaben."
            }
            Self::TlsValidationUnavailable => {
                "Die sichere Verbindung konnte nicht abschließend geprüft werden."
            }
            Self::HttpStatus(_) => {
                "Die Downloadquelle hat die Anfrage nicht erfolgreich beantwortet."
            }
            Self::ResponseTooLarge => "Der Download überschreitet die zulässige Größe.",
            Self::InvalidResponse => "Die Downloadantwort ist unvollständig oder ungültig.",
            Self::DestinationIo => "Der Download konnte nicht vollständig gespeichert werden.",
            Self::UnsupportedPlatform => "Die native Netzwerkverbindung benötigt Windows.",
            Self::Unavailable => "Die native Netzwerkverbindung ist nicht verfügbar.",
        })
    }
}
impl std::error::Error for NetworkError {}

#[derive(Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);
impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadProgress {
    pub bytes_received: u64,
    /// Advertised length, checked against actual bytes before success. None
    /// means indeterminate progress, never a fabricated percentage.
    pub total_bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadResult {
    pub bytes_received: u64,
}

pub struct NetworkClient {
    config: NetworkConfig,
}
impl NetworkClient {
    pub fn new(config: NetworkConfig) -> Self {
        Self { config }
    }
    pub fn config(&self) -> &NetworkConfig {
        &self.config
    }

    /// Run on a native blocking worker. Cancellation is observed before/after
    /// each WinHTTP phase. Progress callbacks execute on that calling worker.
    /// The overall deadline is cooperative: phase timers use its remaining
    /// budget, but Windows may perform internal PAC or multiple sub-operations
    /// within one call. This is not a hard process-level execution deadline.
    pub fn download(
        &self,
        url: &HttpsUrl,
        allowed: &AllowedHosts,
        limits: DownloadLimits,
        cancel: &CancellationToken,
        output: &mut impl Write,
        progress: impl FnMut(DownloadProgress),
    ) -> Result<DownloadResult, NetworkError> {
        if cancel.is_cancelled() {
            return Err(NetworkError::Cancelled);
        }
        if !allowed.permits(url) {
            return Err(NetworkError::SourceNotAllowed);
        }
        #[cfg(windows)]
        {
            windows::download(&self.config, url, allowed, limits, cancel, output, progress)
        }
        #[cfg(not(windows))]
        {
            let _ = (limits, output, progress);
            Err(NetworkError::UnsupportedPlatform)
        }
    }
}

#[cfg(any(windows, test))]
fn check_operation(
    cancel: &CancellationToken,
    deadline: std::time::Instant,
) -> Result<(), NetworkError> {
    if cancel.is_cancelled() {
        Err(NetworkError::Cancelled)
    } else if std::time::Instant::now() >= deadline {
        Err(NetworkError::Timeout)
    } else {
        Ok(())
    }
}

#[cfg(any(windows, test))]
fn finish_phase<T>(
    result: Result<T, NetworkError>,
    cancel: &CancellationToken,
    deadline: std::time::Instant,
) -> Result<T, NetworkError> {
    check_operation(cancel, deadline)?;
    result
}

#[cfg(any(windows, test))]
fn phase_timeout_ms(remaining: std::time::Duration, maximum_ms: u32) -> Result<i32, NetworkError> {
    if remaining.is_zero() {
        return Err(NetworkError::Timeout);
    }
    if maximum_ms == 0 || maximum_ms > i32::MAX as u32 {
        return Err(NetworkError::InvalidConfiguration);
    }
    // Positive sub-millisecond budgets round to one millisecond. Zero would
    // disable WinHTTP's timeout entirely rather than request immediate expiry.
    Ok(remaining.as_millis().clamp(1, u128::from(maximum_ms)) as i32)
}

#[cfg(any(windows, test))]
fn copy_body(
    mut read: impl FnMut(&mut [u8]) -> Result<usize, NetworkError>,
    output: &mut impl Write,
    max_bytes: u64,
    total: Option<u64>,
    cancel: &CancellationToken,
    deadline: std::time::Instant,
    mut progress: impl FnMut(DownloadProgress),
) -> Result<DownloadResult, NetworkError> {
    if total.is_some_and(|total| total > max_bytes) {
        return Err(NetworkError::ResponseTooLarge);
    }
    let mut count = 0;
    let mut buffer = [0; 65_536];
    loop {
        check_operation(cancel, deadline)?;
        let result = read(&mut buffer);
        let length = finish_phase(result, cancel, deadline)?;
        if length == 0 {
            if total.is_some_and(|total| count != total) {
                return Err(NetworkError::InvalidResponse);
            }
            return Ok(DownloadResult {
                bytes_received: count,
            });
        }
        if length > buffer.len() {
            return Err(NetworkError::InvalidResponse);
        }
        count += length as u64;
        if count > max_bytes {
            return Err(NetworkError::ResponseTooLarge);
        }
        if total.is_some_and(|total| count > total) {
            return Err(NetworkError::InvalidResponse);
        }
        output
            .write_all(&buffer[..length])
            .map_err(|_| NetworkError::DestinationIo)?;
        progress(DownloadProgress {
            bytes_received: count,
            total_bytes: total,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Cursor, Read},
        time::{Duration, Instant},
    };

    #[test]
    fn streams_measured_bytes_and_rejects_a_truncated_advertised_body() {
        let mut source = Cursor::new(b"fixture".to_vec());
        let mut bytes = Vec::new();
        let mut observations = Vec::new();
        let result = copy_body(
            |buffer| source.read(buffer).map_err(|_| NetworkError::Offline),
            &mut bytes,
            1024,
            Some(7),
            &CancellationToken::default(),
            Instant::now() + Duration::from_secs(5),
            |progress| observations.push(progress),
        )
        .unwrap();
        assert_eq!(result.bytes_received, 7);
        assert_eq!(bytes, b"fixture");
        assert_eq!(
            observations,
            [DownloadProgress {
                bytes_received: 7,
                total_bytes: Some(7)
            }]
        );
        let mut source = Cursor::new(b"short".to_vec());
        assert_eq!(
            copy_body(
                |buffer| source.read(buffer).map_err(|_| NetworkError::Offline),
                &mut Vec::new(),
                1024,
                Some(7),
                &CancellationToken::default(),
                Instant::now() + Duration::from_secs(5),
                |_| {}
            ),
            Err(NetworkError::InvalidResponse)
        );
    }

    #[test]
    fn rejects_oversize_before_writing_and_observes_cancellation_before_returning_success() {
        let mut source = Cursor::new(b"oversize".to_vec());
        let mut output = Vec::new();
        assert_eq!(
            copy_body(
                |buffer| source.read(buffer).map_err(|_| NetworkError::Offline),
                &mut output,
                4,
                None,
                &CancellationToken::default(),
                Instant::now() + Duration::from_secs(5),
                |_| {}
            ),
            Err(NetworkError::ResponseTooLarge)
        );
        assert!(output.is_empty());
        let token = CancellationToken::default();
        assert_eq!(
            copy_body(
                |_| {
                    token.cancel();
                    Ok(0)
                },
                &mut output,
                1024,
                None,
                &token,
                Instant::now() + Duration::from_secs(5),
                |_| {}
            ),
            Err(NetworkError::Cancelled)
        );
        assert!(output.is_empty());
    }

    #[test]
    fn destination_failure_and_expired_deadline_never_report_success() {
        struct Unwritable;
        impl Write for Unwritable {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "fixture",
                ))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut source = Cursor::new(b"fixture".to_vec());
        assert_eq!(
            copy_body(
                |buffer| source.read(buffer).map_err(|_| NetworkError::Offline),
                &mut Unwritable,
                1024,
                Some(7),
                &CancellationToken::default(),
                Instant::now() + Duration::from_secs(5),
                |_| panic!("failed writes do not produce progress")
            ),
            Err(NetworkError::DestinationIo)
        );
        assert_eq!(
            copy_body(
                |_| panic!("expired operations do not read"),
                &mut Vec::new(),
                1024,
                None,
                &CancellationToken::default(),
                Instant::now(),
                |_| {}
            ),
            Err(NetworkError::Timeout)
        );
    }

    #[test]
    fn cancellation_during_a_failed_read_takes_precedence_over_its_network_error() {
        let token = CancellationToken::default();
        let mut output = Vec::new();
        assert_eq!(
            copy_body(
                |_| {
                    token.cancel();
                    Err(NetworkError::Offline)
                },
                &mut output,
                1024,
                None,
                &token,
                Instant::now() + Duration::from_secs(5),
                |_| panic!("failed reads do not report progress")
            ),
            Err(NetworkError::Cancelled)
        );
        assert!(output.is_empty());
    }

    #[test]
    fn a_failed_phase_preserves_deadline_and_cancellation_priority() {
        let token = CancellationToken::default();
        assert_eq!(
            finish_phase::<()>(Err(NetworkError::Offline), &token, Instant::now()),
            Err(NetworkError::Timeout)
        );
        token.cancel();
        assert_eq!(
            finish_phase::<()>(Err(NetworkError::Offline), &token, Instant::now()),
            Err(NetworkError::Cancelled)
        );
        assert_eq!(
            finish_phase::<()>(Ok(()), &token, Instant::now() + Duration::from_secs(1)),
            Err(NetworkError::Cancelled)
        );
    }

    #[test]
    fn phase_timeouts_fit_remaining_budget_without_zero_or_excessive_values() {
        assert_eq!(
            phase_timeout_ms(Duration::from_secs(1), 30_000).unwrap(),
            1000
        );
        assert_eq!(
            phase_timeout_ms(Duration::from_secs(60), 30_000).unwrap(),
            30_000
        );
        assert_eq!(
            phase_timeout_ms(Duration::from_nanos(1), 30_000).unwrap(),
            1
        );
        assert_eq!(
            phase_timeout_ms(Duration::ZERO, 30_000),
            Err(NetworkError::Timeout)
        );
        assert_eq!(
            phase_timeout_ms(Duration::from_secs(1), 0),
            Err(NetworkError::InvalidConfiguration)
        );
    }
}
