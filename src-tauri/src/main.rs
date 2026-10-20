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
#[cfg(target_os = "macos")]
const RELEASES_URL: &str = "https://github.com/bvdon/payback/releases/latest";
#[cfg(target_os = "macos")]
const LATEST_RELEASE_API: &str = "https://api.github.com/repos/bvdon/payback/releases/latest";

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

/// Open a web page in the default browser.
#[cfg(target_os = "macos")]
fn open_url(url: &str) {
    if let Err(e) = std::process::Command::new("open").arg(url).spawn() {
        eprintln!("couldn't open {url}: {e}");
    }
}

/// Ask GitHub for the newest release, using the curl that ships with macOS
/// (so the app needs no HTTP or TLS library of its own).
#[cfg(target_os = "macos")]
fn fetch_latest_version() -> Result<String, String> {
    let out = std::process::Command::new("/usr/bin/curl")
        .args(["--silent", "--show-error", "--fail", "--location", "--max-time", "15"])
        .args(["--header", "Accept: application/vnd.github+json", "--user-agent", "Payback"])
        .arg(LATEST_RELEASE_API)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    payback_core::updates::latest_version(&String::from_utf8_lossy(&out.stdout)).map_err(|e| e.to_string())
}

/// "Check for Updates…": compare this version with the latest release and
/// say so, offering the download page when there's something newer.
#[cfg(target_os = "macos")]
fn check_for_updates(app: &tauri::AppHandle) {
    use payback_core::updates::{compare, UpdateStatus};
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

    let current = app.package_info().version.to_string();
    let app = app.clone();
    // The network request runs off the main thread so the menu stays responsive.
    std::thread::spawn(move || {
        let dialog = app.dialog();
        match fetch_latest_version().and_then(|latest| compare(&current, &latest).map_err(|e| e.to_string())) {
            Ok(UpdateStatus::UpToDate) => {
                dialog
                    .message(format!("Payback {current} is the newest version. There’s nothing to update."))
                    .title("You’re up to date")
                    .kind(MessageDialogKind::Info)
                    .buttons(MessageDialogButtons::OkCustom("OK".into()))
                    .show(|_| {});
            }
            Ok(UpdateStatus::Available(latest)) => {
                dialog
                    .message(format!(
                        "Payback {latest} is available. You have {current}.\n\nOpen the download page to get the new version? Your loans stay as they are when you update."
                    ))
                    .title("A new version of Payback is available")
                    .kind(MessageDialogKind::Info)
                    .buttons(MessageDialogButtons::OkCancelCustom("Open Download Page".into(), "Not Now".into()))
                    .show(|open| {
                        if open {
                            open_url(RELEASES_URL);
                        }
                    });
            }
            Err(e) => {
                eprintln!("update check failed: {e}");
                dialog
                    .message(format!(
                        "Payback couldn’t reach GitHub to check for a newer version. Check your internet connection and try again, or look at the releases page yourself. You have version {current}."
                    ))
                    .title("Couldn’t check for updates")
                    .kind(MessageDialogKind::Warning)
                    .buttons(MessageDialogButtons::OkCancelCustom("Open Releases Page".into(), "OK".into()))
                    .show(|open| {
                        if open {
                            open_url(RELEASES_URL);
                        }
                    });
            }
        }
    });
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

            // The standard Mac menu, plus "Check for Updates…" under About.
            #[cfg(target_os = "macos")]
            {
                use tauri::menu::{Menu, MenuItem, MenuItemKind};
                let menu = Menu::default(app.handle())?;
                if let Some(MenuItemKind::Submenu(app_menu)) = menu.items()?.into_iter().next() {
                    let updates = MenuItem::with_id(app, "check-updates", "Check for Updates…", true, None::<&str>)?;
                    app_menu.insert(&updates, 1)?;
                }
                app.set_menu(menu)?;
                app.on_menu_event(|app, event| {
                    if event.id() == "check-updates" {
                        check_for_updates(app);
                    }
                });
            }

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
