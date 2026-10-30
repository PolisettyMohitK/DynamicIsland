//! Game mode: while a full-screen or borderless game (or video) is in front, everything
//! steps aside. The island hides and its WebView2 is suspended (no scripts, no drawing),
//! Choco stops listening, the pet's window closes, and the pollers slow right down.
//! When you leave the game it all resumes exactly where it was.
//!
//! Emits `game-mode` (bool).

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

static ON: AtomicBool = AtomicBool::new(false);

const CHECK: Duration = Duration::from_millis(500);
/// Checks in a row before switching: quick to step aside, slower to come back
/// (alt-tabbing out for a second shouldn't wake everything).
const ENTER_AFTER: u32 = 2;
const LEAVE_AFTER: u32 = 4;
/// How long the slowed-down pollers sleep while a game is on.
pub const IDLE_POLL: Duration = Duration::from_secs(2);

pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut streak = 0u32;
        loop {
            std::thread::sleep(CHECK);
            let full = crate::island::fullscreen_app_active();
            streak = if full != on() { streak + 1 } else { 0 };
            if streak >= if full { ENTER_AFTER } else { LEAVE_AFTER } {
                streak = 0;
                set(&app, full);
            }
        }
    });
}

fn set(app: &AppHandle, gaming: bool) {
    ON.store(gaming, Ordering::Relaxed);
    if let Some(main) = app.get_webview_window("main") {
        if gaming {
            let _ = main.hide();
            suspend(&main, true);
        } else {
            suspend(&main, false);
            let hidden = app.state::<crate::island::UserHidden>().0.load(Ordering::Relaxed);
            if !hidden {
                let _ = main.show();
            }
        }
    }
    let _ = app.emit("game-mode", gaming);
}

/// WebView2's own sleep: scripts and timers stop and memory is trimmed, but the page
/// stays as it is. It needs the webview marked invisible first.
#[cfg(target_os = "windows")]
fn suspend(window: &WebviewWindow, sleep: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_3;
    use webview2_com::TrySuspendCompletedHandler;
    use windows_core::Interface;

    let _ = window.with_webview(move |wv| unsafe {
        let controller = wv.controller();
        let _ = controller.SetIsVisible(!sleep);
        let Ok(core) = controller.CoreWebView2() else { return };
        let Ok(core) = core.cast::<ICoreWebView2_3>() else { return };
        if sleep {
            let _ = core.TrySuspend(&TrySuspendCompletedHandler::create(Box::new(|_, _| Ok(()))));
        } else {
            let _ = core.Resume();
        }
    });
}

#[cfg(not(target_os = "windows"))]
fn suspend(_: &WebviewWindow, _: bool) {}
