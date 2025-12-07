mod commands;
mod mistral_client;
mod tray;

use tauri::{Manager, Listener};
use tauri_plugin_store::StoreExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Initialize tray
            tray::create_tray(app)?;
            
            // Handle requests from tray to clear history
            let app_handle = app.handle().clone();
            app.listen("request-clear-history", move |_| {
                let store = app_handle.store("conversations.json");
                 if let Ok(store) = store {
                     let _ = store.clear();
                     let _ = store.save();
                 }
            });
            
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Prevent window from closing, hide it instead
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::send_message,
            commands::create_conversation,
            commands::get_conversations,
            commands::delete_conversation,
            commands::rename_conversation,
            commands::clear_history,
            commands::get_app_settings,
            commands::update_settings,
            commands::test_api_connection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
