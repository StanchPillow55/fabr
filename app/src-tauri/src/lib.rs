use fabr_core::ops::{Desktop, Done, dispatch, url_allowed};
use fabr_core::workspace::Workspace;
use fabr_core::{AppDirs, ErrorDto, FabrError, client_error};
use tauri::{Manager, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

fn effect(ws: &Workspace, d: Desktop) -> Result<(), FabrError> {
    let io = |e: tauri_plugin_opener::Error| FabrError::Io(e.to_string());
    match d {
        Desktop::OpenPath(r) => tauri_plugin_opener::open_path(ws.local_path(&r)?, None::<&str>).map_err(io),
        Desktop::Reveal(r) => {
            let p = std::path::PathBuf::from(&r.path);
            if !ws.revealable(&p) {
                return Err(FabrError::BadRequest("only the log file can be revealed".into()));
            }
            tauri_plugin_opener::reveal_item_in_dir(p).map_err(io)
        }
        Desktop::OpenUrl(r) => {
            let u = r.url.trim();
            if !url_allowed(u) {
                return Err(FabrError::BadRequest("only http and https links can be opened".into()));
            }
            tauri_plugin_opener::open_url(u, None::<&str>).map_err(io)
        }
    }
}

#[tauri::command(rename_all = "snake_case", async)]
fn call(ws: State<'_, Workspace>, name: String, req: serde_json::Value) -> Result<serde_json::Value, ErrorDto> {
    let out = match dispatch(&ws, &name, req) {
        Ok(Done::Json(v)) => Ok(v),
        Ok(Done::Desktop(d)) => effect(&ws, d).map(|()| serde_json::Value::Null),
        Err(e) => Err(e),
    };
    out.map_err(|e| client_error(&name, e))
}

pub fn run() {
    let log_path = match fabr_core::logging::init_default(cfg!(debug_assertions)) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("fabr: logging disabled: {}", e.message());
            None
        }
    };
    log::info!(
        "fabr app {} starting; log {}",
        env!("CARGO_PKG_VERSION"),
        log_path.as_ref().map_or("(none)".into(), |p| p.display().to_string())
    );
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            match AppDirs::resolve().workspace(AppDirs::env_root()) {
                Ok(ws) => {
                    app.manage(ws);
                }
                Err(e) => {
                    log::error!("setup failed: {}", e.message());
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.hide();
                    }
                    let where_ = log_path.as_ref().map_or("no log file".into(), |p| format!("Log: {}", p.display()));
                    let handle = app.handle().clone();
                    app.dialog()
                        .message(format!("Fabr could not start:\n\n{}\n\n{where_}", e.message()))
                        .title("Fabr")
                        .kind(MessageDialogKind::Error)
                        .show(move |_| handle.exit(1));
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![call])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        log::error!("tauri failed: {e}");
        eprintln!("fabr: {e}");
        std::process::exit(1);
    }
}
