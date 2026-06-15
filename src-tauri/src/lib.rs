use std::{
    fmt::Write as _,
    path::PathBuf,
    process::Command,
    sync::{Arc, Mutex, RwLock},
};

use reqbook::{preview, workspace};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

pub struct TauriState {
    pub workspace_root: Arc<RwLock<PathBuf>>,
    pub server_port: Arc<Mutex<u16>>,
}

fn new_write_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).expect("failed to create desktop session token");
    let mut token = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(&mut token, "{byte:02x}");
    }
    token
}

#[tauri::command]
async fn get_server_url(state: State<'_, Arc<TauriState>>) -> Result<String, String> {
    let port = *state.server_port.lock().unwrap();
    Ok(format!("http://127.0.0.1:{port}"))
}

#[tauri::command]
async fn open_workspace(
    path: String,
    state: State<'_, Arc<TauriState>>,
    app: AppHandle,
) -> Result<(), String> {
    let new_root = PathBuf::from(&path);
    let name = workspace::workspace_name(&new_root)
        .or_else(|| {
            new_root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| path.clone());
    *state.workspace_root.write().unwrap() = new_root.clone();
    workspace::save_to_history(&new_root, &name);
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.set_title(&format!("Reqbook — {name}"));
    }
    Ok(())
}

#[tauri::command]
fn open_external(url: String) -> Result<(), String> {
    const ALLOWED_URLS: [&str; 3] = [
        "https://markapidown.net/out/feedback",
        "https://markapidown.net/out/bug",
        "https://markapidown.net/out/star",
    ];

    if !ALLOWED_URLS.contains(&url.as_str()) {
        return Err("external URL is not allowed".to_string());
    }

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        #[cfg(target_os = "macos")]
        let status = Command::new("open").arg(&url).status();

        #[cfg(target_os = "linux")]
        let status = Command::new("xdg-open").arg(&url).status();

        #[cfg(target_os = "windows")]
        let status = Command::new("cmd").args(["/C", "start", "", &url]).status();

        match status {
            Ok(exit) if exit.success() => Ok(()),
            Ok(exit) => Err(format!("system browser exited with {exit}")),
            Err(error) => Err(format!("failed to open system browser: {error}")),
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err("opening external URLs is not supported on this platform".to_string())
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let initial_root = workspace::collection_root(None)
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| workspace::collection_root(None));

            let workspace_root = Arc::new(RwLock::new(initial_root.clone()));
            let server_port = Arc::new(Mutex::new(7700u16));

            let root_clone = workspace_root.clone();
            let port_clone = server_port.clone();
            let app_handle = app.handle().clone();
            let app_handle_for_picker = app.handle().clone();
            let write_token = new_write_token();

            let pick_folder_fn: preview::PickFolderFn = Arc::new(move || {
                let (tx, rx) = tokio::sync::oneshot::channel();
                app_handle_for_picker
                    .dialog()
                    .file()
                    .pick_folder(move |path| {
                        let _ = tx.send(path.map(|p| p.to_string()));
                    });
                rx
            });

            tauri::async_runtime::spawn(async move {
                let (tx, rx) = tokio::sync::oneshot::channel::<u16>();
                let root_for_server = root_clone.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = preview::run_with_ready_options(
                        root_for_server,
                        "127.0.0.1",
                        7700,
                        "dev",
                        false,
                        preview::PreviewOptions {
                            pick_folder: Some(pick_folder_fn),
                            write_token: Some(write_token),
                        },
                        move |port| {
                            *port_clone.lock().unwrap() = port;
                            let _ = tx.send(port);
                        },
                    )
                    .await
                    {
                        eprintln!("rqb-desktop: server error: {e}");
                    }
                });

                if let Ok(port) = rx.await {
                    let url = format!("http://127.0.0.1:{port}");
                    if let Some(win) = app_handle.get_webview_window("main") {
                        if let Ok(parsed) = url.parse() {
                            let _ = win.navigate(parsed);
                        }
                    }
                }
            });

            app.manage(Arc::new(TauriState {
                workspace_root,
                server_port,
            }));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_server_url,
            open_workspace,
            open_external,
        ])
        .run(tauri::generate_context!())
        .expect("error while running rqb-desktop");
}
