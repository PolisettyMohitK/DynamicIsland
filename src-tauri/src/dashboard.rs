//! The dashboard: a plain window with every setting (src/routes/dashboard). It opens when you
//! launch the app, but not when it starts with Windows. It's only built when opened, and
//! closing it frees it; the island keeps running, and launching the app again or the tray
//! brings it back.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const LABEL: &str = "dashboard";
/// Passed by the Windows autostart entry, so logging in doesn't open the dashboard.
pub const QUIET_START: &str = "--minimized";

fn create(app: &AppHandle) -> tauri::Result<tauri::WebviewWindow> {
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("dashboard".into()))
        .title("Islet")
        .inner_size(760.0, 620.0)
        .min_inner_size(400.0, 360.0)
        .center()
        .visible(false)
        .build()
}

/// Opens the dashboard (building it if it's closed) in front of everything. Windows won't hand
/// focus to a background process on request, so it's briefly made topmost to get above
/// whatever is in front.
pub fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL).or_else(|| create(app).ok()) {
        let _ = window.show();
        // Windows swaps a process's first show for the way it was started, and the installer's
        // "Run Islet" starts it hidden; so if that one didn't take, show it again.
        if !window.is_visible().unwrap_or(true) {
            let _ = window.show();
        }
        let _ = window.unminimize();
        let _ = window.set_always_on_top(true);
        let _ = window.set_focus();
        let _ = window.set_always_on_top(false);
    }
}
