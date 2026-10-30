#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::{
    path::PathBuf,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
    sync::oneshot,
};

#[derive(Default)]
struct Session(
    Mutex<Option<(u64, Option<oneshot::Sender<()>>)>>,
    AtomicBool,
);

#[tauri::command]
async fn diagnose(
    app: tauri::AppHandle,
    state: tauri::State<'_, Session>,
    id: u64,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let encoded = serde_json::to_vec(&request).map_err(|_| "Ungültige Anfrage.")?;
    if encoded.len() > 2 * 1024 * 1024 {
        return Err("Eingabe überschreitet 2 MiB.".into());
    }
    let (tx, rx) = oneshot::channel();
    {
        let mut active = state
            .0
            .lock()
            .map_err(|_| "Sitzungsstatus nicht verfügbar.")?;
        if state.1.load(Ordering::Acquire) {
            return Err("App wird geschlossen.".into());
        }
        if active.is_some() {
            return Err("Eine Diagnose läuft bereits.".into());
        }
        *active = Some((id, Some(tx)));
    }
    let result = run_engine(&app, encoded, rx).await;
    let mut active = state
        .0
        .lock()
        .map_err(|_| "Sitzungsstatus nicht verfügbar.")?;
    if active
        .as_ref()
        .is_some_and(|(active_id, _)| *active_id == id)
    {
        *active = None;
    }
    result
}

async fn run_engine(
    app: &tauri::AppHandle,
    encoded: Vec<u8>,
    cancel: oneshot::Receiver<()>,
) -> Result<serde_json::Value, String> {
    let root: PathBuf = if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/engine")
    } else {
        app.path()
            .resource_dir()
            .map_err(|_| "Ressourcenpfad fehlt.")?
            .join("engine")
    };
    run_engine_at(root, encoded, cancel).await
}

async fn run_engine_at(
    root: PathBuf,
    encoded: Vec<u8>,
    cancel: oneshot::Receiver<()>,
) -> Result<serde_json::Value, String> {
    let bundled_java = root.join(if cfg!(windows) {
        "runtime/bin/java.exe"
    } else {
        "runtime/bin/java"
    });
    let java = if bundled_java.is_file() {
        bundled_java
    } else if cfg!(debug_assertions) {
        PathBuf::from("java")
    } else {
        return Err("Gebündelte Java-Laufzeit fehlt. Installation prüfen.".into());
    };
    let mut command = Command::new(java);
    command
        .args([
            "-Xmx256m",
            "-Dfile.encoding=UTF-8",
            "-Dcom.sun.security.enableAIAcaIssuers=false",
            "-jar",
        ])
        .arg(root.join("inspect-engine.jar"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let mut child = command
        .spawn()
        .map_err(|_| "Diagnosekern konnte nicht gestartet werden.")?;
    let work = async {
        let mut stdin = child.stdin.take().ok_or("IPC-Eingabe fehlt.")?;
        stdin
            .write_all(&encoded)
            .await
            .map_err(|_| "IPC-Eingabe fehlgeschlagen.")?;
        stdin
            .shutdown()
            .await
            .map_err(|_| "IPC-Abschluss fehlgeschlagen.")?;
        drop(stdin);
        let mut bytes = Vec::new();
        child
            .stdout
            .take()
            .ok_or("IPC-Ausgabe fehlt.")?
            .take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| "IPC-Ausgabe fehlgeschlagen.")?;
        if bytes.len() > 32 * 1024 * 1024 {
            return Err("Diagnoseantwort überschreitet das Limit.".to_string());
        }
        let status = child.wait().await.map_err(|_| "Diagnosekern beendet.")?;
        if !status.success() {
            return Err("Diagnosekern meldet einen Fehler.".into());
        }
        serde_json::from_slice(&bytes).map_err(|_| "Diagnoseantwort ist ungültig.".into())
    };
    tokio::select! {
        result = work => result,
        _ = cancel => Err("Diagnose abgebrochen.".into()),
        _ = tokio::time::sleep(Duration::from_secs(150)) => Err("Zeitlimit der Diagnose erreicht.".into())
    }
}

#[tauri::command]
async fn cancel(state: tauri::State<'_, Session>, id: u64) -> Result<(), String> {
    let mut active = state
        .0
        .lock()
        .map_err(|_| "Sitzungsstatus nicht verfügbar.")?;
    if let Some((active_id, sender)) = active.as_mut() {
        if *active_id == id {
            if let Some(tx) = sender.take() {
                let _ = tx.send(());
            }
        }
    }
    Ok(())
}

#[tauri::command]
async fn save_report(
    app: tauri::AppHandle,
    content: String,
    format: String,
) -> Result<bool, String> {
    if !["json", "html"].contains(&format.as_str()) || content.len() > 32 * 1024 * 1024 {
        return Err("Berichtsformat oder Größe unzulässig.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let selected = app
            .dialog()
            .file()
            .set_file_name(format!("fbforge-inspect.{}", format))
            .add_filter("Diagnosebericht", &[format.as_str()])
            .blocking_save_file();
        let Some(selected) = selected else {
            return Ok(false);
        };
        let path = selected
            .into_path()
            .map_err(|_| "Lokalen Speicherort wählen.")?;
        std::fs::write(path, content)
            .map_err(|_| "Bericht konnte nicht gespeichert werden.".to_string())?;
        Ok(true)
    })
    .await
    .map_err(|_| "Speicherdialog fehlgeschlagen.".to_string())?
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Session::default())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<Session>();
                state.1.store(true, Ordering::Release);
                if let Ok(mut active) = state.0.lock() {
                    if let Some((_, sender)) = active.as_mut() {
                        api.prevent_close();
                        if let Some(sender) = sender.take() {
                            let _ = sender.send(());
                        }
                        let app = window.app_handle().clone();
                        tauri::async_runtime::spawn(async move {
                            loop {
                                let idle = app
                                    .state::<Session>()
                                    .0
                                    .lock()
                                    .map(|active| active.is_none())
                                    .unwrap_or(true);
                                if idle {
                                    app.exit(0);
                                    break;
                                }
                                tokio::time::sleep(Duration::from_millis(10)).await;
                            }
                        });
                    }
                };
            }
        })
        .invoke_handler(tauri::generate_handler![diagnose, cancel, save_report])
        .run(tauri::generate_context!())
        .expect("FBForge Inspect konnte nicht gestartet werden");
}

#[cfg(test)]
mod tests {
    use super::*;
    fn engine_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/engine")
    }
    #[tokio::test]
    async fn bundled_engine_round_trip() {
        let (_sender, receiver) = oneshot::channel();
        let request = serde_json::json!({"protocolVersion":1,"operation":"schema","environment":"TEST","apiVersion":"v3","input":{"text":"{\"type\":\"object\"}"}});
        let result = run_engine_at(
            engine_root(),
            serde_json::to_vec(&request).unwrap(),
            receiver,
        )
        .await
        .unwrap();
        assert_eq!(result["ok"], true);
        assert_eq!(result["report"]["protocolVersion"], 1);
        assert_eq!(result["report"]["operation"], "schema");
    }
    #[tokio::test]
    async fn private_input_never_returns_in_engine_error() {
        let (_sender, receiver) = oneshot::channel();
        let request = serde_json::json!({"protocolVersion":1,"operation":"keys","environment":"TEST","apiVersion":"v3","input":{"text":"{\"kty\":\"RSA\",\"d\":\"fixture-private-marker\"}"}});
        let result = run_engine_at(
            engine_root(),
            serde_json::to_vec(&request).unwrap(),
            receiver,
        )
        .await
        .unwrap();
        assert_eq!(result["ok"], false);
        assert!(!result.to_string().contains("fixture-private-marker"));
    }
    #[tokio::test]
    async fn cancellation_terminates_the_process_request() {
        let (sender, receiver) = oneshot::channel();
        sender.send(()).unwrap();
        let result = run_engine_at(engine_root(), b"{}".to_vec(), receiver).await;
        assert_eq!(result.unwrap_err(), "Diagnose abgebrochen.");
    }

    #[tokio::test]
    async fn bundled_engine_accepts_bom_prefixed_xsd() {
        let (_sender, receiver) = oneshot::channel();
        let request = serde_json::json!({"protocolVersion":1,"operation":"schema","environment":"TEST","apiVersion":"v3","input":{"text":"\u{feff}<xs:schema xmlns:xs='http://www.w3.org/2001/XMLSchema' version='2.3.1'/>"}});
        let result = run_engine_at(
            engine_root(),
            serde_json::to_vec(&request).unwrap(),
            receiver,
        )
        .await
        .unwrap();
        assert_eq!(result["ok"], true);
        assert_eq!(result["report"]["data"]["dialect"], "XSD");
        assert_eq!(result["report"]["data"]["schemaVersion"], "2.3.1");
    }

    #[tokio::test]
    async fn bundled_engine_rejects_excess_schema_nodes() {
        let (_sender, receiver) = oneshot::channel();
        let schema: serde_json::Map<String, serde_json::Value> = (0..4000)
            .map(|i| (format!("field{i}"), serde_json::json!({})))
            .collect();
        let request = serde_json::json!({"protocolVersion":1,"operation":"schema","environment":"TEST","apiVersion":"v3","input":{"text":serde_json::Value::Object(schema).to_string()}});
        let result = run_engine_at(
            engine_root(),
            serde_json::to_vec(&request).unwrap(),
            receiver,
        )
        .await
        .unwrap();
        assert_eq!(result["ok"], false);
        assert_eq!(result["error"], "Strukturlimit erreicht.");
    }
}
