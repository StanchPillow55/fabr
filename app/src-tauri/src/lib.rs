mod agent;

use agent::AgentHandle;
use fabr_core::ops::{Desktop, Done, dispatch, url_allowed};
use fabr_core::workspace::Workspace;
use fabr_core::{AppDirs, ErrorDto, FabrError, client_error};
use std::sync::Mutex;
use tauri::{Manager, RunEvent, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

/// Tauri-managed state holding the hosted worker's lifecycle handle.
///
/// Wrapped in a `Mutex<Option<_>>` so the app-exit callback can take exclusive
/// access and `take()` the handle exactly once: `AgentHandle::shutdown` needs
/// `&mut self`, and `take()` guards against a double shutdown if the exit event
/// is observed more than once.
type AgentState = Mutex<Option<AgentHandle>>;

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
                    // The worker needs its own owned `Workspace` (it is not
                    // `Clone`, and `app.manage(ws)` moved the data-layer one into
                    // Tauri state). Resolve a second one the same cheap way; both
                    // point at the same fabric space on disk, which is correct —
                    // the data layer and the worker share one space, exactly as
                    // two hosts would. A failure here must not fail the app: log
                    // a WARN and launch without a worker (Requirements 1.1, 3.1).
                    let agent_state: AgentState = match AppDirs::resolve().workspace(AppDirs::env_root()) {
                        Ok(agent_ws) => Mutex::new(Some(AgentHandle::spawn(agent_ws))),
                        Err(e) => {
                            log::warn!("agent worker not started ({}); the app runs without a worker", e.message());
                            Mutex::new(None)
                        }
                    };
                    app.manage(agent_state);
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
        .build(tauri::generate_context!());
    let app = match result {
        Ok(app) => app,
        Err(e) => {
            log::error!("tauri failed: {e}");
            eprintln!("fabr: {e}");
            std::process::exit(1);
        }
    };
    // Observe app exit (window-close or a programmatic `handle.exit`) and stop
    // the worker: flip its stop flag and join the thread so the runner's clean
    // shutdown removes only this process's registry/_backend entries
    // (Requirement 10.6). `ExitRequested` covers both paths in this Tauri
    // version. The handle is taken out of the `Mutex<Option<_>>` so shutdown
    // runs at most once even if the event is seen again.
    app.run(|app_handle, event| {
        if let RunEvent::ExitRequested { .. } = event
            && let Some(state) = app_handle.try_state::<AgentState>()
            && let Ok(mut guard) = state.lock()
            && let Some(mut handle) = guard.take()
        {
            handle.shutdown();
        }
    });
}
