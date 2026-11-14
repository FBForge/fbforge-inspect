//! Disposable bytes, never persisted execution authority. Native callers use a
//! separate cache mutex and borrow StateStore only for prepare/final approval.
mod files;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

use super::{StateStore, StoreError};
use crate::{
    network::{
        AllowedHosts, CancellationToken, DownloadLimits, DownloadProgress, DownloadResult,
        HttpsUrl, NetworkClient, NetworkError,
    },
    trust::{TrustError, TrustStore, VerifiedRelease},
};
use semver::Version;
use std::{
    fmt,
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    sync::Arc,
    time::Duration,
};

const MAX_BYTES: u64 = 32 * 1024 * 1024 * 1024;
const MAX_FILES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageCacheLimits {
    bytes: u64,
    files: usize,
}
impl Default for PackageCacheLimits {
    fn default() -> Self {
        Self {
            bytes: 4 * 1024 * 1024 * 1024,
            files: 16,
        }
    }
}
impl PackageCacheLimits {
    /// Native policy only, independent of advertised download lengths.
    pub fn new(bytes: u64, files: usize) -> Result<Self, PackageCacheError> {
        if bytes == 0 || bytes > MAX_BYTES || files == 0 || files > MAX_FILES {
            return Err(PackageCacheError::InvalidLimits);
        }
        Ok(Self { bytes, files })
    }
}

pub struct PackageCache {
    directory: files::Directory,
    _cache_lock: File,
    store_lock: Arc<File>,
    limits: PackageCacheLimits,
}
impl StateStore {
    /// Creates an independent cache owner. It retains this exact OS store lock
    /// without duplicating the OS handle, but never borrows the StateStore.
    pub fn package_cache(
        &self,
        limits: PackageCacheLimits,
    ) -> Result<PackageCache, PackageCacheError> {
        self.ensure_ready().map_err(PackageCacheError::Store)?;
        let directory = files::Directory::open(&self.root)?;
        let lock = directory.open_lock()?;
        lock.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => PackageCacheError::Busy,
            std::fs::TryLockError::Error(_) => PackageCacheError::Io,
        })?;
        let mut cache = PackageCache {
            directory,
            _cache_lock: lock,
            store_lock: Arc::clone(&self._lock),
            limits,
        };
        let entries = cache.directory.clean_and_list()?;
        cache.directory.reserve(entries, 0, 0, limits)?;
        Ok(cache)
    }
}
impl PackageCache {
    fn pair(&self, store: &StateStore) -> Result<(), PackageCacheError> {
        if Arc::ptr_eq(&self.store_lock, &store._lock) {
            Ok(())
        } else {
            Err(PackageCacheError::WrongStore)
        }
    }
    /// Opens a cache hit as untrusted staging, or creates bounded exclusive
    /// download staging. Neither branch hashes bytes while borrowing the store.
    pub fn prepare<'a>(
        &'a mut self,
        store: &mut StateStore,
        trust: &TrustStore,
        now: u64,
        tool_id: &str,
        version: &Version,
        cancel: &CancellationToken,
    ) -> Result<PackageStaging<'a>, PackageCacheError> {
        self.pair(store)?;
        check_cancel(cancel)?;
        let release = store
            .load_release(trust, now, tool_id, version)
            .map_err(PackageCacheError::Store)?;
        require_available(&release)?;
        if release.package_size() > self.limits.bytes {
            return Err(PackageCacheError::Full);
        }
        let name = format!(
            "{}.pkg",
            crate::trust::encoding::encode_hex(release.package_sha256())
        );
        let entries = self.directory.clean_and_list()?;
        if entries.iter().any(|entry| entry.name == name) {
            let file = self.directory.open_file(&name, false)?;
            let size = file.metadata().map_err(|_| PackageCacheError::Io)?.len();
            if size > self.limits.bytes {
                return Err(PackageCacheError::Full);
            }
            self.directory.reserve(
                entries.into_iter().filter(|e| e.name != name).collect(),
                size,
                1,
                self.limits,
            )?;
            return Ok(PackageStaging {
                cache: Some(self),
                file: Some(file),
                name,
                release,
                cached: true,
                written: 0,
                phase: Phase::Writing,
            });
        }
        self.directory
            .reserve(entries, release.package_size(), 1, self.limits)?;
        check_cancel(cancel)?;
        let (name, file) = self.directory.create_staging()?;
        Ok(PackageStaging {
            cache: Some(self),
            file: Some(file),
            name,
            release,
            cached: false,
            written: 0,
            phase: Phase::Writing,
        })
    }
}

enum Phase {
    Writing,
    Checked(files::Stamp),
    Failed,
}
/// This guard borrows only PackageCache. State/administrative operations remain
/// possible while the network writes and while verify_bytes hashes the file.
pub struct PackageStaging<'a> {
    cache: Option<&'a mut PackageCache>,
    file: Option<File>,
    name: String,
    release: VerifiedRelease,
    cached: bool,
    written: u64,
    phase: Phase,
}
impl<'a> PackageStaging<'a> {
    pub fn is_cached(&self) -> bool {
        self.cached
    }
    /// Native transport may derive the URL/byte limit from these metadata.
    /// This is neither an execution grant nor a persistent trust decision.
    pub fn release(&self) -> &VerifiedRelease {
        &self.release
    }
    /// Download only the verified release's GitHub asset into this private
    /// staging file. Cache hits recheck bytes without a network request or
    /// invented network progress. This seals bytes, not metadata or execution
    /// authority: finish still revalidates the current store, trust and clock.
    /// The ten-minute transport budget is cooperative and excludes local hashing.
    pub fn download_and_verify_bytes(
        &mut self,
        client: &NetworkClient,
        cancel: &CancellationToken,
        progress: impl FnMut(DownloadProgress),
    ) -> Result<(), PackageCacheError> {
        if !matches!(self.phase, Phase::Writing) {
            self.phase = Phase::Failed;
            return Err(PackageCacheError::InvalidState);
        }
        if self.cached {
            return self.verify_bytes(None, cancel);
        }
        // Pass the whole transfer result to the existing sealing contract, even
        // when source/limit setup fails. A failed attempt cannot later finish.
        let transfer = (|| {
            let url = HttpsUrl::parse(&self.release.download_url())?;
            let hosts = AllowedHosts::new([
                "github.com",
                "release-assets.githubusercontent.com",
                "objects.githubusercontent.com",
            ])?;
            let limits =
                DownloadLimits::new(self.release.package_size(), Duration::from_secs(600))?;
            client.download(&url, &hosts, limits, cancel, self, progress)
        })();
        self.verify_bytes(Some(transfer), cancel)
    }
    /// Long I/O phase, without a StateStore borrow. Hits require None; newly
    /// downloaded bytes require the entire transfer Result, including failures.
    /// Success seals the guard: any subsequent Write poisons it permanently.
    pub fn verify_bytes(
        &mut self,
        transfer: Option<Result<DownloadResult, NetworkError>>,
        cancel: &CancellationToken,
    ) -> Result<(), PackageCacheError> {
        if !matches!(self.phase, Phase::Writing) {
            self.phase = Phase::Failed;
            return Err(PackageCacheError::InvalidState);
        }
        self.phase = Phase::Failed;
        check_cancel(cancel)?;
        match (self.cached, transfer) {
            (_, Some(Err(error))) => return Err(PackageCacheError::Network(error)),
            (false, Some(Ok(result))) if result.bytes_received == self.written => {}
            (true, None) => {}
            _ => return Err(PackageCacheError::InvalidState),
        }
        let cache = self
            .cache
            .as_deref()
            .ok_or(PackageCacheError::InvalidState)?;
        let file = self.file.as_mut().ok_or(PackageCacheError::InvalidState)?;
        cache.directory.validate(file)?;
        if !self.cached {
            file.sync_all().map_err(|_| PackageCacheError::Io)?;
        }
        let before = platform::stamp(file)?;
        file.seek(SeekFrom::Start(0))
            .map_err(|_| PackageCacheError::Io)?;
        let checked = self.release.verify_package(CancelReader { file, cancel });
        check_cancel(cancel)?;
        checked.map_err(PackageCacheError::Trust)?;
        cache.directory.validate(file)?;
        let after = platform::stamp(file)?;
        if before != after {
            return Err(PackageCacheError::Corrupt);
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|_| PackageCacheError::Io)?;
        self.phase = Phase::Checked(after);
        Ok(())
    }
    /// Short final authority check after verify_bytes. The callback is a native
    /// clock read performed here, after the long hash/flush phase has completed.
    pub fn finish(
        mut self,
        store: &mut StateStore,
        trust: &TrustStore,
        clock: impl FnOnce() -> u64,
        cancel: &CancellationToken,
    ) -> Result<CachedPackage<'a>, PackageCacheError> {
        let cache = self
            .cache
            .as_deref()
            .ok_or(PackageCacheError::InvalidState)?;
        cache.pair(store)?;
        check_cancel(cancel)?;
        let Phase::Checked(expected) = &self.phase else {
            return Err(PackageCacheError::InvalidState);
        };
        let file = self.file.as_ref().ok_or(PackageCacheError::InvalidState)?;
        cache.directory.validate(file)?;
        if platform::stamp(file)? != *expected {
            return Err(PackageCacheError::Corrupt);
        }
        let current = store
            .load_release(
                trust,
                clock(),
                self.release.tool_id(),
                self.release.version(),
            )
            .map_err(PackageCacheError::Store)?;
        require_available(&current)?;
        if current.app_id() != self.release.app_id()
            || current.package_size() != self.release.package_size()
            || current.package_sha256() != self.release.package_sha256()
        {
            return Err(PackageCacheError::Corrupt);
        }
        check_cancel(cancel)?;
        if !self.cached {
            let destination = format!(
                "{}.pkg",
                crate::trust::encoding::encode_hex(current.package_sha256())
            );
            cache.directory.publish(file, &self.name, &destination)?;
            self.name = destination;
            self.cached = true;
        }
        let file = self.file.take().ok_or(PackageCacheError::InvalidState)?;
        let cache = self.cache.take().ok_or(PackageCacheError::InvalidState)?;
        Ok(CachedPackage {
            file,
            _cache: cache,
            release: current,
        })
    }
}
impl Write for PackageStaging<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let valid = matches!(self.phase, Phase::Writing)
            && !self.cached
            && self
                .written
                .checked_add(bytes.len() as u64)
                .is_some_and(|n| {
                    n <= self.release.package_size()
                        && self
                            .cache
                            .as_deref()
                            .is_some_and(|cache| n <= cache.limits.bytes)
                });
        if !valid {
            self.phase = Phase::Failed;
            return Err(io::Error::other(
                "Das Paketstaging ist gesperrt oder überschreitet seine Grenze.",
            ));
        }
        match self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("Das Paketstaging ist nicht verfügbar."))?
            .write(bytes)
        {
            Ok(0) if !bytes.is_empty() => {
                self.phase = Phase::Failed;
                Err(io::Error::other(
                    "Das Paket wurde nicht vollständig geschrieben.",
                ))
            }
            Ok(n) => {
                self.written += n as u64;
                Ok(n)
            }
            Err(_) => {
                self.phase = Phase::Failed;
                Err(io::Error::other(
                    "Das Paket konnte nicht gespeichert werden.",
                ))
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        if !matches!(self.phase, Phase::Writing) || self.cached {
            self.phase = Phase::Failed;
            return Err(io::Error::other("Das Paketstaging ist gesperrt."));
        }
        let result = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("Das Paketstaging ist nicht verfügbar."))?
            .flush();
        if result.is_err() {
            self.phase = Phase::Failed;
            return Err(io::Error::other(
                "Das Paket konnte nicht gespeichert werden.",
            ));
        }
        Ok(())
    }
}
impl Drop for PackageStaging<'_> {
    fn drop(&mut self) {
        if !self.cached {
            if let Some(file) = &self.file {
                if let Some(cache) = self.cache.as_deref() {
                    let _ = cache.directory.remove(file, &self.name);
                }
            }
        }
    }
}
/// Read-only package bytes held under the cache/store-lock lifetime. It conveys
/// no Authenticode result, installer permission or future metadata freshness.
///
/// ```compile_fail
/// use fbforge_suite_core::storage::CachedPackage;
/// use std::io::Write;
/// fn overwrite(package: &mut CachedPackage<'_>) {
///     package.write_all(b"changed").unwrap();
/// }
/// ```
///
/// ```compile_fail
/// use fbforge_suite_core::{network::CancellationToken, storage::StateStore, trust::TrustStore};
/// fn close_cache_early(mut store: StateStore, trust: &TrustStore) {
///     let mut cache = store.package_cache(Default::default()).unwrap();
///     let cancel = CancellationToken::default();
///     let mut staging = cache.prepare(&mut store, trust, 300,
///         "fbforge-connect", &"0.3.0".parse().unwrap(), &cancel).unwrap();
///     staging.verify_bytes(None, &cancel).unwrap();
///     let _package = staging.finish(&mut store, trust, || 300, &cancel).unwrap();
///     drop(cache);
/// }
/// ```
pub struct CachedPackage<'a> {
    file: File,
    _cache: &'a mut PackageCache,
    // Identity binding from finish, not a claim of future metadata freshness.
    release: VerifiedRelease,
}
impl CachedPackage<'_> {
    pub(crate) fn matches_release(&self, release: &VerifiedRelease) -> bool {
        self.release.tool_id() == release.tool_id()
            && self.release.app_id() == release.app_id()
            && self.release.version() == release.version()
            && self.release.package_size() == release.package_size()
            && self.release.package_sha256() == release.package_sha256()
    }

    // Only the native Authenticode adapter borrows the existing file. This is
    // never exposed as a public File/raw handle and does not reopen by path.
    #[cfg(windows)]
    pub(crate) fn file_mut(&mut self) -> &mut File {
        &mut self.file
    }
    #[cfg(windows)]
    pub(crate) fn file_ref(&self) -> &File {
        &self.file
    }

    // Only consuming installer preparation calls this. No old publisher proof
    // survives the sharing transition; all bytes and publisher evidence must be
    // established again before a PreparedInstaller can be returned.
    #[cfg(windows)]
    pub(crate) fn prepare_installer_read(&mut self) -> Result<(), PackageCacheError> {
        self._cache.directory.validate(&self.file)?;
        platform::prepare_installer_read(&mut self.file)
    }
}
// Dropck must retain the cache-owner borrow until this file is closed, even
// when the caller never reads the guard again. A field borrow alone can end
// early under NLL and otherwise allow the owner/OS locks to disappear first.
impl Drop for CachedPackage<'_> {
    fn drop(&mut self) {}
}
impl Read for CachedPackage<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.file.read(buffer)
    }
}
impl Seek for CachedPackage<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.file.seek(position)
    }
}
struct CancelReader<'a> {
    file: &'a mut File,
    cancel: &'a CancellationToken,
}
impl Read for CancelReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.cancel.is_cancelled() {
            return Err(io::Error::other("Die Paketprüfung wurde abgebrochen."));
        }
        self.file.read(buffer)
    }
}
fn check_cancel(cancel: &CancellationToken) -> Result<(), PackageCacheError> {
    if cancel.is_cancelled() {
        Err(PackageCacheError::Cancelled)
    } else {
        Ok(())
    }
}
fn require_available(release: &VerifiedRelease) -> Result<(), PackageCacheError> {
    if release.is_available() {
        Ok(())
    } else {
        Err(PackageCacheError::Trust(TrustError::WithdrawnRelease))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PackageCacheError {
    InvalidLimits,
    Busy,
    WrongStore,
    InvalidState,
    Full,
    UnsafePath,
    Io,
    Corrupt,
    Cancelled,
    Indeterminate,
    UnsupportedPlatform,
    Network(NetworkError),
    Trust(TrustError),
    Store(StoreError),
}
impl fmt::Display for PackageCacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidLimits => "Die Grenzen der Paketablage sind ungültig.",
            Self::Busy => "Die Paketablage wird bereits verwendet.",
            Self::WrongStore => "Die Paketablage gehört zu einer anderen Datenablage.",
            Self::InvalidState => "Das Paket wurde nicht vollständig geprüft.",
            Self::Full => "Die Paketablage hat ihre zulässige Grenze erreicht.",
            Self::UnsafePath => "Die Paketablage ist nicht ausreichend geschützt.",
            Self::Io => "Die Paketdatei konnte nicht vollständig gelesen oder geschrieben werden.",
            Self::Corrupt => "Die Paketdatei wurde verändert oder ist beschädigt.",
            Self::Cancelled => "Die Paketprüfung wurde abgebrochen.",
            Self::Indeterminate => {
                "Der Stand der Paketablage ist unklar. Prüfen Sie die Ablage erneut."
            }
            Self::UnsupportedPlatform => {
                "Die sichere Paketablage wird auf dieser Plattform nicht unterstützt."
            }
            Self::Network(error) => return error.fmt(f),
            Self::Trust(error) => return error.fmt(f),
            Self::Store(error) => return error.fmt(f),
        })
    }
}
impl std::error::Error for PackageCacheError {}
