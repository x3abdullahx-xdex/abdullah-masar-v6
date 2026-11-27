pub mod commands;
pub mod db;
pub mod logical_time;
pub mod sync;

use rusqlite::Connection;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::Manager;

use crate::commands::AppDbState;
use crate::db::migrations::init_database;

static MINIMIZE_TO_TRAY: AtomicBool = AtomicBool::new(true);

#[tauri::command]
fn set_minimize_to_tray(enabled: bool) {
    MINIMIZE_TO_TRAY.store(enabled, Ordering::Relaxed);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_masar_timer::init());

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.unminimize();
                let _ = win.show();
                let _ = win.set_focus();
            }
        }));
    }

    builder
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            std::fs::create_dir_all(&app_dir)?;
            let db_path = app_dir.join("abdullah_masar_v6.db");
            let conn = Connection::open(db_path)?;
            let device_id = init_database(&conn)?;
            app.manage(AppDbState {
                conn: Mutex::new(conn),
                device_id,
            });

            // Desktop System Tray + Minimize to Tray on Close
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                use tauri::menu::{Menu, MenuItem};
                use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

                let show_i = MenuItem::with_id(app, "show", "فتح لوحة المسار", true, None::<&str>)?;
                let hide_i = MenuItem::with_id(app, "hide", "إخفاء للتراي", true, None::<&str>)?;
                let quit_i = MenuItem::with_id(app, "quit", "خروج نهائي", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show_i, &hide_i, &quit_i])?;

                let _tray = TrayIconBuilder::new()
                    .icon(app.default_window_icon().unwrap().clone())
                    .tooltip("لوحة المسار اليومي V6")
                    .menu(&menu)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "show" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.unminimize();
                                let _ = w.set_focus();
                            }
                        }
                        "hide" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.hide();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.unminimize();
                                let _ = w.set_focus();
                            }
                        }
                    })
                    .build(app)?;

                if let Some(win) = app.get_webview_window("main") {
                    let win_clone = win.clone();
                    win.on_window_event(move |event| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                            if MINIMIZE_TO_TRAY.load(Ordering::Relaxed) {
                                api.prevent_close();
                                let _ = win_clone.hide();
                            }
                        }
                    });
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set_minimize_to_tray,
            commands::get_logical_today,
            commands::get_day_bundle,
            commands::create_task,
            commands::toggle_task_done,
            commands::reschedule_task,
            commands::cancel_task_with_excuse,
            commands::toggle_prayer,
            commands::toggle_weekly_shield,
            commands::log_focus_session,
            commands::add_distraction,
            commands::list_notes,
            commands::save_note,
            commands::delete_note,
            commands::get_monthly_telemetry,
            commands::import_v5_backup,
            commands::run_gist_sync,
            commands::fetch_and_save_ui_update,
            commands::load_hot_ui,
            commands::clear_hot_ui
        ])
        .run(tauri::generate_context!())
        .expect("error while running Abdullah Masar V6");
}
