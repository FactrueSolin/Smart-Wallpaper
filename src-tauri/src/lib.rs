use serde::Serialize;

pub mod wallpaper_manager;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BackendStatus {
    integrated: bool,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppStatus {
    name: &'static str,
    version: &'static str,
    backend: BackendStatus,
}

#[tauri::command]
fn app_status() -> AppStatus {
    let (integrated, backend_message) = match wallpaper_manager::WallpaperManager::new(
        wallpaper_manager::SwiftAppKitBridgeBackend::new(),
    )
    .refresh()
    {
        Ok(states) => (
            true,
            format!(
                "Wallpaper manager is ready. {} screen(s) detected.",
                states.len()
            ),
        ),
        Err(err) => (
            false,
            format!("Wallpaper module loaded. Backend status: {}", err),
        ),
    };

    AppStatus {
        name: "智能壁纸",
        version: env!("CARGO_PKG_VERSION"),
        backend: BackendStatus {
            integrated,
            message: backend_message,
        },
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_status])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
