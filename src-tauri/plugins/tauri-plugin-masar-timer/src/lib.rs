use serde::{Deserialize, Serialize};
use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct StartTimerRequest {
    pub duration_secs: u64,
    pub label: String,
}

#[tauri::command]
async fn start_foreground_timer<R: Runtime>(
    _app: tauri::AppHandle<R>,
    _payload: StartTimerRequest,
) -> Result<(), String> {
    // On Android, invokes TimerPlugin.kt -> FocusForegroundService.kt
    // On Desktop, no-op (WebView Zen timer + OS notification handles countdown)
    Ok(())
}

#[tauri::command]
async fn stop_foreground_timer<R: Runtime>(_app: tauri::AppHandle<R>) -> Result<(), String> {
    Ok(())
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("masar-timer")
        .invoke_handler(tauri::generate_handler![
            start_foreground_timer,
            stop_foreground_timer
        ])
        .build()
}
