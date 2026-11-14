//! Startup-only coordination. This grants no installer or updater authority.
use fbforge_suite_coordination::{Acquisition, AppIdentity, Lease};

pub const APP_ID: &str = "de.fbforge.inspect";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupError {
    Busy,
    NeedsRescan,
    Unavailable,
}

pub fn acquire() -> Result<Lease, StartupError> {
    let identity = AppIdentity::new(APP_ID).map_err(|_| StartupError::Unavailable)?;
    require_clean_lease(Lease::try_acquire(&identity))
}

fn require_clean_lease(outcome: Acquisition) -> Result<Lease, StartupError> {
    match outcome {
        Acquisition::Acquired(lease) => Ok(lease),
        // Drop on this owning thread; a later explicit start may acquire afresh.
        Acquisition::AbandonedNeedsRescan(_lease) => Err(StartupError::NeedsRescan),
        Acquisition::Busy => Err(StartupError::Busy),
        Acquisition::Unavailable(_) => Err(StartupError::Unavailable),
    }
}

impl StartupError {
    pub fn message(self) -> &'static str {
        match self {
            Self::Busy => "FBForge Inspect ist bereits geöffnet oder wird verwaltet. Wechseln Sie zur geöffneten Anwendung. Warten Sie bei einer laufenden Installation, bis sie abgeschlossen ist.",
            Self::NeedsRescan => "Die vorherige App-Koordination wurde unterbrochen. FBForge Inspect wurde vorsorglich nicht gestartet. Prüfen Sie zuerst laufende Arbeit und gespeicherte Daten. Eine Wiederherstellung ist noch nicht bestätigt.",
            Self::Unavailable => "Die gemeinsame Windows-Koordination ist nicht verfügbar. FBForge Inspect wurde nicht gestartet. Versuchen Sie es später erneut.",
        }
    }
}

pub fn show_error(error: StartupError) {
    show_message(error.message());
}

pub fn show_update_error() {
    show_message("Der Start des Update-Installers konnte nicht bestätigt werden. FBForge Inspect bleibt beendet. Starten Sie die Anwendung erneut und prüfen Sie das Update.");
}

fn show_message(message: &str) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            MessageBoxW, MB_ICONWARNING, MB_OK, MB_TASKMODAL,
        };
        let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
        let title: Vec<u16> = "FBForge Inspect".encode_utf16().chain(Some(0)).collect();
        // SAFETY: both terminated buffers live through this synchronous call.
        // The native startup dialog receives no renderer, SID, path or OS error.
        if unsafe {
            MessageBoxW(
                Default::default(),
                text.as_ptr(),
                title.as_ptr(),
                MB_OK | MB_ICONWARNING | MB_TASKMODAL,
            )
        } != 0
        {
            return;
        }
    }
    eprintln!("{message}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use fbforge_suite_coordination::{Acquisition, UnavailableReason};

    #[test]
    fn busy_start_never_enters_the_application() {
        assert_eq!(
            require_clean_lease(Acquisition::Busy).unwrap_err(),
            StartupError::Busy
        );
    }

    #[test]
    fn every_unavailable_reason_blocks_start_without_private_details() {
        for reason in [
            UnavailableReason::UnsupportedPlatform,
            UnavailableReason::IdentityUnavailable,
            UnavailableReason::UntrustedObject,
            UnavailableReason::OperatingSystem,
        ] {
            assert_eq!(
                require_clean_lease(Acquisition::Unavailable(reason)).unwrap_err(),
                StartupError::Unavailable
            );
        }
    }

    #[test]
    fn app_identity_matches_the_native_tauri_configuration() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(config["identifier"], APP_ID);
        assert_eq!(
            fbforge_suite_coordination::AppIdentity::new(APP_ID)
                .unwrap()
                .as_str(),
            APP_ID
        );
    }

    #[cfg(windows)]
    #[test]
    fn successful_start_retains_ownership_until_the_same_thread_drops_it() {
        let identity = fbforge_suite_coordination::AppIdentity::new(&format!(
            "example.inspect.startup.{}.held",
            std::process::id()
        ))
        .unwrap();
        let lease =
            require_clean_lease(fbforge_suite_coordination::Lease::try_acquire(&identity)).unwrap();
        assert!(matches!(
            fbforge_suite_coordination::Lease::try_acquire(&identity),
            Acquisition::Busy
        ));
        drop(lease);
        assert!(
            require_clean_lease(fbforge_suite_coordination::Lease::try_acquire(&identity)).is_ok()
        );
    }

    #[cfg(windows)]
    #[test]
    fn abandoned_start_is_rejected_but_reacquisition_loses_crash_history() {
        let identity = fbforge_suite_coordination::AppIdentity::new(&format!(
            "example.inspect.startup.{}.abandoned",
            std::process::id()
        ))
        .unwrap();
        let other = identity.clone();
        std::thread::spawn(move || {
            let lease = require_clean_lease(fbforge_suite_coordination::Lease::try_acquire(&other))
                .unwrap();
            std::mem::forget(lease);
        })
        .join()
        .unwrap();
        assert_eq!(
            require_clean_lease(fbforge_suite_coordination::Lease::try_acquire(&identity))
                .unwrap_err(),
            StartupError::NeedsRescan
        );
        // The OS abandonment signal is consumed, not a durable recovery marker.
        assert!(
            require_clean_lease(fbforge_suite_coordination::Lease::try_acquire(&identity)).is_ok()
        );
    }
}
