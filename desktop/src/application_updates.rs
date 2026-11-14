//! Native-owned update identity, verified bytes and explicit installation permission.
use crate::{update_policy as policy, update_transport, Session};
use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};
use tauri::{Manager, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::{Update, UpdaterExt};

#[cfg(windows)]
pub(crate) type InstallerQueue =
    std::sync::Arc<crate::update_handoff::InstallerHandoff<PendingInstaller>>;
#[cfg(windows)]
pub(crate) struct PendingInstaller {
    release: Update,
    bytes: Vec<u8>,
}
#[cfg(windows)]
impl PendingInstaller {
    pub(crate) fn install(self) -> Result<(), ()> {
        self.release.install(self.bytes).map_err(|_| ())
    }
}

pub const PUBLIC_KEY: &str = include_str!("../update-public-key.txt");

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    configured: bool,
    supported: bool,
    phase: &'static str,
    version: Option<String>,
    notes: String,
    downloaded: usize,
    total: Option<u64>,
}
impl Default for UpdateStatus {
    fn default() -> Self {
        Self {
            configured: policy::public_key(PUBLIC_KEY).is_ok(),
            supported: cfg!(all(windows, target_arch = "x86_64")),
            phase: "idle",
            version: None,
            notes: String::new(),
            downloaded: 0,
            total: None,
        }
    }
}
#[derive(Default)]
struct Pending {
    status: UpdateStatus,
    release: Option<Update>,
    bytes: Option<Vec<u8>>,
}
#[derive(Default)]
pub struct ApplicationUpdates {
    pending: Mutex<Pending>,
    cancel: AtomicBool,
    #[cfg(windows)]
    installer: InstallerQueue,
}
impl ApplicationUpdates {
    #[cfg(windows)]
    pub(crate) fn with_installer(installer: InstallerQueue) -> Self {
        Self {
            installer,
            ..Self::default()
        }
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Pending>, &'static str> {
        self.pending.lock().map_err(|_| "update_unavailable")
    }
}

#[tauri::command]
pub fn application_update_status(
    updates: State<'_, ApplicationUpdates>,
) -> Result<UpdateStatus, &'static str> {
    Ok(updates.lock()?.status.clone())
}

#[tauri::command]
pub async fn check_application_update(
    app: tauri::AppHandle,
    updates: State<'_, ApplicationUpdates>,
) -> Result<UpdateStatus, &'static str> {
    {
        let mut pending = updates.lock()?;
        if !pending.status.supported {
            return Err("update_platform_unsupported");
        }
        if !pending.status.configured {
            return Err("update_not_configured");
        }
        if matches!(
            pending.status.phase,
            "checking" | "downloading" | "installing"
        ) {
            return Err("update_busy");
        }
        *pending = Pending::default();
        pending.status.phase = "checking";
    }
    let result = async {
        let raw = update_transport::manifest().await?;
        if !policy::manifest(&raw, &app.package_info().version.to_string())? {
            return Ok(None);
        }
        let updater = app
            .updater_builder()
            .target("windows-x86_64")
            .pubkey(PUBLIC_KEY.trim())
            .timeout(Duration::from_secs(600))
            .version_comparator(|current, release| {
                release.version.pre.is_empty() && release.version > current
            });
        // run_return already performs cleanup before the main thread installs.
        // Retain no default AppHandle cleanup callback in the deferred Update.
        #[cfg(windows)]
        let updater = updater.on_before_exit(|| {});
        updater
            .build()
            .map_err(|_| "update_manifest_invalid")?
            .restore_update(raw)
            .map_err(|_| "update_manifest_invalid")
    }
    .await;
    let mut pending = updates.lock()?;
    pending.status.phase = "idle";
    match result {
        Ok(Some(release)) => {
            pending.status.phase = "available";
            pending.status.version = Some(release.version.clone());
            pending.status.notes = release.body.clone().unwrap_or_default();
            pending.release = Some(release);
        }
        Ok(None) => pending.status.phase = "current",
        Err(error) => return Err(error),
    }
    Ok(pending.status.clone())
}

#[tauri::command]
pub async fn download_application_update(
    updates: State<'_, ApplicationUpdates>,
    version: String,
) -> Result<UpdateStatus, &'static str> {
    let release = {
        let mut pending = updates.lock()?;
        if pending.status.phase != "available" || pending.status.version.as_ref() != Some(&version)
        {
            return Err("update_not_ready");
        }
        let release = pending.release.clone().ok_or("update_not_ready")?;
        updates.cancel.store(false, Ordering::Release);
        pending.status.phase = "downloading";
        pending.status.downloaded = 0;
        pending.status.total = None;
        release
    };
    let result = async {
        let bytes = update_transport::package(
            release.download_url.as_str(),
            &updates.cancel,
            |done, total| {
                if let Ok(mut pending) = updates.lock() {
                    pending.status.downloaded = done;
                    pending.status.total = total;
                }
            },
        )
        .await?;
        if updates.cancel.load(Ordering::Acquire) {
            return Err("update_cancelled");
        }
        policy::verify(&bytes, &release.signature, PUBLIC_KEY, &version)?;
        release
            .verify(&bytes)
            .map_err(|_| "update_signature_invalid")?;
        if updates.cancel.load(Ordering::Acquire) {
            return Err("update_cancelled");
        }
        Ok(bytes)
    }
    .await;
    let mut pending = updates.lock()?;
    pending.status.phase = "available";
    match result {
        Ok(bytes) => {
            pending.bytes = Some(bytes);
            pending.status.phase = "downloaded";
        }
        Err(error) => return Err(error),
    }
    Ok(pending.status.clone())
}

#[tauri::command]
pub fn cancel_application_update(updates: State<'_, ApplicationUpdates>) {
    updates.cancel.store(true, Ordering::Release);
}

async fn confirm_install(app: &tauri::AppHandle, version: &str) -> Result<bool, &'static str> {
    let window = app.get_webview_window("main").ok_or("update_unavailable")?;
    let app = app.clone();
    let version = version.to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog().message(format!("FBForge Inspect {version} wurde heruntergeladen und seine Signatur geprüft.\n\nExportieren Sie benötigte Prüfaufträge und Ergebnisse vor dem Update. Nicht gespeicherte Inhalte dieser Sitzung gehen beim Beenden verloren.\n\nJetzt die Anwendung beenden und den Installer starten?"))
            .parent(&window).title("Update installieren?").kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom("Installieren und neu starten".into(), "Zurück zur Anwendung".into()))
            .blocking_show()
    }).await.map_err(|_| "update_unavailable")
}

#[tauri::command]
pub async fn install_application_update(
    app: tauri::AppHandle,
    engine: State<'_, Session>,
    updates: State<'_, ApplicationUpdates>,
    version: String,
) -> Result<UpdateStatus, &'static str> {
    let release = {
        let mut pending = updates.lock()?;
        if pending.status.phase != "downloaded" || pending.status.version.as_ref() != Some(&version)
        {
            return Err("update_not_ready");
        }
        let release = pending.release.clone().ok_or("update_not_ready")?;
        pending.status.phase = "installing";
        release
    };
    let result = async {
        if !confirm_install(&app, &version).await? {
            return Err("update_cancelled");
        }
        // Serialize the final readiness check with new diagnoses and save dialogs.
        {
            let active = engine.0.lock().map_err(|_| "update_unavailable")?;
            if active.is_some() || engine.2.load(Ordering::Acquire) != 0 {
                return Err("update_activity_pending");
            }
            if engine.1.swap(true, Ordering::AcqRel) {
                return Err("update_activity_pending");
            }
        }
        let bytes = updates.lock()?.bytes.take().ok_or("update_not_ready")?;
        #[cfg(windows)]
        {
            let status = updates.lock()?.status.clone();
            updates
                .installer
                .submit(PendingInstaller { release, bytes })
                .map_err(|_| "update_install_failed")?;
            Ok(Some(status))
        }
        #[cfg(not(windows))]
        {
            tokio::task::spawn_blocking(move || release.install(bytes))
                .await
                .map_err(|_| "update_install_failed")?
                .map_err(|_| "update_install_failed")?;
            Ok(None::<UpdateStatus>)
        }
    }
    .await;
    let result = crate::update_handoff::finish_attempt(result, &engine.1);
    let result = match result {
        Ok(Some(status)) => {
            // No state access after requesting exit: cleanup may begin immediately.
            app.exit(0);
            return Ok(status);
        }
        result => result.map(|_| ()),
    };
    let mut pending = updates.lock()?;
    pending.status.phase = if pending.bytes.is_some() {
        "downloaded"
    } else {
        "available"
    };
    result?;
    Ok(pending.status.clone())
}
