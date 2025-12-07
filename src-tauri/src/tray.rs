use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    App, Runtime,
    Manager,
    Listener,
    Emitter,
};

pub fn create_tray<R: Runtime>(app: &App<R>) -> tauri::Result<tauri::tray::TrayIcon<R>> {
    let toggle_i = MenuItem::with_id(app, "toggle", "Afficher/Masquer", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quitter", true, None::<&str>)?;
    let settings_i = MenuItem::with_id(app, "settings", "Paramètres", true, None::<&str>)?;
    let clear_i = MenuItem::with_id(app, "clear_history", "Effacer l'historique", true, None::<&str>)?;
    
    let menu = Menu::with_items(app, &[&toggle_i, &settings_i, &clear_i, &quit_i])?;

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| {
            match event.id.as_ref() {
                "quit" => app.exit(0),
                "toggle" => {
                    if let Some(window) = app.get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
                "settings" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                        let _ = window.emit("open-settings", ());
                    }
                }
                "clear_history" => {
                     if let Some(window) = app.get_webview_window("main") {
                        let _ = window.emit("request-clear-history", ());
                    }
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    ..
                } => {
                    let app = tray.app_handle();
                    if let Some(window) = app.get_webview_window("main") {
                         if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                    }
                }
                _ => {}
            }
        })
        .build(app)
}
