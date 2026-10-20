// Payback desktop shell: a window for the UI plus file access for exports,
// imports and backups. All app logic lives in payback-core.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use payback_core::api::{self, Api};
use payback_core::store::read_text;
use payback_core::{Json, Store};
use tauri::{Manager, State};

const KEEP_BACKUPS: usize = 14;
const BACKUP_EVERY_SECS: u64 = 20 * 3600;

struct AppState {
    api: Mutex<Api>,
    data_dir: PathBuf,
    /// Set when the data file exists but couldn't be read. Every command then
    /// reports it, so nothing is saved over the file the person still needs.
    open_error: Option<String>,
}

impl AppState {
    fn api(&self) -> MutexGuard<'_, Api> {
        self.api.lock().unwrap_or_else(|p| p.into_inner())
    }
    fn backups_dir(&self) -> PathBuf {
        self.data_dir.join("backups")
    }
}

fn io_error(msg: impl Into<String>) -> String {
    Json::obj().with("kind", "io").with("message", msg.into()).stringify()
}

// -- commands -----------------------------------------------------------------

/// The single entry point for the UI: `cmd` names a core command, `args` is JSON.
#[tauri::command]
fn call(state: State<'_, AppState>, cmd: String, args: String) -> Result<String, String> {
    if let Some(e) = &state.open_error {
        return Err(e.clone());
    }
    state.api().call(&cmd, &args)
}

/// Run an export command and write the file to `path` (chosen in a save dialog).
#[tauri::command]
fn export_file(state: State<'_, AppState>, cmd: String, args: String, path: String) -> Result<String, String> {
    if let Some(e) = &state.open_error {
        return Err(e.clone());
    }
    let parsed = Json::parse(&args).map_err(|e| api::error_json(&e))?;
    let export = state.api().export(&cmd, &parsed).map_err(|e| api::error_json(&e))?;
    std::fs::write(&path, &export.bytes).map_err(|e| io_error(format!("Couldn't save the file: {e}")))?;
    Ok(path)
}

/// Read a `.payback` file the user picked for import.
#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    read_text(&PathBuf::from(path)).map_err(|e| api::error_json(&e))
}

#[tauri::command]
fn backup_now(state: State<'_, AppState>) -> Result<String, String> {
    if let Some(e) = &state.open_error {
        return Err(e.clone());
    }
    let dir = state.backups_dir();
    state.api().store().backup_now(&dir).map(|p| p.display().to_string()).map_err(|e| api::error_json(&e))
}

#[tauri::command]
fn restore_backup(state: State<'_, AppState>, path: String) -> Result<String, String> {
    if let Some(e) = &state.open_error {
        return Err(e.clone());
    }
    let dir = state.backups_dir();
    let result = state.api().store_mut().restore_from(&PathBuf::from(path), &dir);
    result.map(|p| p.display().to_string()).map_err(|e| api::error_json(&e))
}

/// Where things live on disk, for the Settings screen.
#[tauri::command]
fn app_paths(state: State<'_, AppState>) -> String {
    let data = state.api().store().path().map(|p| p.display().to_string());
    Json::obj()
        .with("dataDir", state.data_dir.display().to_string())
        .with("dataFile", data)
        .with("backupsDir", state.backups_dir().display().to_string())
        .stringify()
}

/// Show a file or folder in Finder / File Explorer.
#[tauri::command]
fn reveal(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg("-R").arg(&path).spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("explorer").arg(format!("/select,{path}")).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let result = std::process::Command::new("xdg-open")
        .arg(PathBuf::from(&path).parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from(&path)))
        .spawn();
    result.map(|_| ()).map_err(|e| io_error(format!("Couldn't open the folder: {e}")))
}

// -- app --------------------------------------------------------------------------

fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| show_main(app)))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let (store, open_error) = match Store::open(&data_dir.join("payback.json")) {
                Ok(s) => (s, None),
                Err(e) => (Store::in_memory(), Some(api::error_json(&e))),
            };
            let state = AppState { api: Mutex::new(Api::new(store)), data_dir, open_error };
            let backup = state.api().store().auto_backup(&state.backups_dir(), KEEP_BACKUPS, BACKUP_EVERY_SECS);
            if let Err(e) = backup {
                eprintln!("automatic backup failed: {e}");
            }
            app.manage(state);

            // If the app stays open for days, keep taking the daily backup.
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(30 * 60));
                let state = handle.state::<AppState>();
                let dir = state.backups_dir();
                let result = state.api().store().auto_backup(&dir, KEEP_BACKUPS, BACKUP_EVERY_SECS);
                if let Err(e) = result {
                    eprintln!("automatic backup failed: {e}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            call,
            export_file,
            read_text_file,
            backup_now,
            restore_backup,
            app_paths,
            reveal
        ])
        .run(tauri::generate_context!())
        .expect("error while starting Payback");
}
