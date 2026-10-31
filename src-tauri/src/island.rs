// Window placement, hover tracking, click-through, hiding and
// Caps Lock events for the island window.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

// Logical size of the transparent window: the largest state (a long answer) plus room for its shadow.
pub const WINDOW_WIDTH: f64 = 900.0;
pub const WINDOW_HEIGHT: f64 = 640.0;

const POLL: Duration = Duration::from_millis(30);
const HIDDEN_POLL: Duration = Duration::from_millis(500);
const HIDE_EVERY: u32 = 10; // polls between checks of whether to hide (~300ms)

/// Logical size of the island's interactive area, top-centred in the window.
pub struct HitSize(pub Mutex<(f64, f64)>);

/// Set from the tray menu.
pub struct UserHidden(pub Arc<AtomicBool>);

/// Whether the cursor is over the island, as last emitted in `island-hover`.
pub struct Hovered(pub AtomicBool);

#[tauri::command]
pub fn set_hit_size(state: State<HitSize>, width: f64, height: f64) {
    *state.0.lock().unwrap() = (width, height);
}

/// The island never takes focus, except while you type into it (adding a task).
#[tauri::command]
pub fn set_typing(window: WebviewWindow, on: bool) -> Result<(), String> {
    window.set_focusable(on).map_err(|e| e.to_string())?;
    if on {
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Lets a freshly loaded frontend catch up with the hover state it missed.
#[tauri::command]
pub fn is_hovered(state: State<Hovered>) -> bool {
    state.0.load(Ordering::Relaxed)
}

/// Put the window at the top centre of the primary monitor.
pub fn place(window: &WebviewWindow) -> tauri::Result<()> {
    let monitor = window.primary_monitor()?.or(window.current_monitor()?);
    window.set_size(tauri::LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT))?;
    if let Some(monitor) = monitor {
        let scale = monitor.scale_factor();
        let width = (WINDOW_WIDTH * scale).round() as i32;
        let x = monitor.position().x + (monitor.size().width as i32 - width) / 2;
        window.set_position(tauri::PhysicalPosition::new(x, monitor.position().y))?;
    }
    window.set_ignore_cursor_events(true)
}

pub fn spawn_watcher(app: AppHandle, window: WebviewWindow) {
    std::thread::spawn(move || {
        let mut hovered = false;
        let mut visible = true;
        let mut caps = caps_lock_on();
        let mut tick = 0u32;
        loop {
            // Hidden (by you or by a game): nothing to hover, so check rarely.
            std::thread::sleep(if visible { POLL } else { HIDDEN_POLL });
            tick = tick.wrapping_add(1);

            if !visible || tick % HIDE_EVERY == 0 {
                let hidden = app.state::<UserHidden>().0.load(Ordering::Relaxed);
                // Game mode shows and hides the window itself (it also suspends the page).
                let want = !hidden && !crate::game::on();
                if want != visible {
                    visible = want;
                    if !crate::game::on() {
                        let _ = if want { window.show() } else { window.hide() };
                    }
                }
            }
            if !visible {
                continue;
            }

            let inside = cursor_inside(&app, &window);
            if inside != hovered {
                hovered = inside;
                app.state::<Hovered>().0.store(inside, Ordering::Relaxed);
                let _ = window.set_ignore_cursor_events(!inside);
                let _ = app.emit("island-hover", inside);
            }

            let now = caps_lock_on();
            if now != caps {
                caps = now;
                let _ = app.emit("island-caps", now);
            }
        }
    });
}

fn cursor_inside(app: &AppHandle, window: &WebviewWindow) -> bool {
    let (Ok(pos), Ok(size), Ok(scale)) =
        (window.outer_position(), window.outer_size(), window.scale_factor())
    else {
        return false;
    };
    let (w, h) = *app.state::<HitSize>().0.lock().unwrap();
    let (cx, cy) = cursor_pos();
    let half = w * scale / 2.0;
    let centre = pos.x as f64 + size.width as f64 / 2.0;
    let x = cx as f64;
    let y = cy as f64;
    x >= centre - half && x <= centre + half && y >= pos.y as f64 && y <= pos.y as f64 + h * scale
}

#[cfg(target_os = "windows")]
pub(crate) fn cursor_pos() -> (i32, i32) {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut p = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut p) };
    (p.x, p.y)
}

#[cfg(target_os = "windows")]
fn caps_lock_on() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CAPITAL};
    unsafe { GetKeyState(VK_CAPITAL as i32) & 1 == 1 }
}

/// True when a game, video or presentation fills the screen, including
/// borderless-windowed games that cover their whole monitor.
#[cfg(target_os = "windows")]
pub(crate) fn fullscreen_app_active() -> bool {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows_sys::Win32::UI::Shell::{
        SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE,
        QUNS_RUNNING_D3D_FULL_SCREEN,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowRect,
    };

    unsafe {
        let mut state = 0;
        if SHQueryUserNotificationState(&mut state) == 0
            && matches!(
                state,
                QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE
            )
        {
            return true;
        }

        let fg = GetForegroundWindow();
        if fg.is_null() {
            return false;
        }
        let mut class = [0u16; 64];
        let len = GetClassNameW(fg, class.as_mut_ptr(), class.len() as i32) as usize;
        let class = String::from_utf16_lossy(&class[..len]);
        if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd") {
            return false; // the desktop itself
        }

        let mut r: RECT = std::mem::zeroed();
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetWindowRect(fg, &mut r) == 0
            || GetMonitorInfoW(MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST), &mut info) == 0
        {
            return false;
        }
        let m = info.rcMonitor;
        r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn cursor_pos() -> (i32, i32) {
    (-1, -1)
}

#[cfg(not(target_os = "windows"))]
fn caps_lock_on() -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn fullscreen_app_active() -> bool {
    false
}
