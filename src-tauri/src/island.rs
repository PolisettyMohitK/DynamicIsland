// Window placement, hover tracking, click-through, hiding and
// Caps Lock events for the island window. Dragging the island moves it to the nearest screen
// edge (top, left or right), where it docks; `island-dock` says which, `island-dragging`
// whether it's being carried.
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

// Logical size of the transparent window at the top: the largest state (a long answer) plus
// room for its shadow. On a side it's SIDE_WIDTH wide and as tall as the work area.
pub const WINDOW_WIDTH: f64 = 900.0;
pub const WINDOW_HEIGHT: f64 = 640.0;
pub const SIDE_WIDTH: f64 = 480.0;
const DOCK_FILE: &str = "island_dock.txt";
/// How often the window follows the cursor while it's carried.
const DRAG_POLL: Duration = Duration::from_millis(7);

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dock {
    Top,
    Left,
    Right,
}

impl Dock {
    const ALL: [Dock; 3] = [Dock::Top, Dock::Left, Dock::Right];

    fn name(self) -> &'static str {
        match self {
            Dock::Top => "top",
            Dock::Left => "left",
            Dock::Right => "right",
        }
    }
}

static DOCK: AtomicU8 = AtomicU8::new(0);
static DRAGGING: AtomicBool = AtomicBool::new(false);

fn dock() -> Dock {
    Dock::ALL[DOCK.load(Ordering::Relaxed) as usize]
}

const POLL: Duration = Duration::from_millis(30);
const HIDDEN_POLL: Duration = Duration::from_millis(500);
const HIDE_EVERY: u32 = 10; // polls between checks of whether to hide (~300ms)

/// The island's interactive area in the window, logical (x, y, width, height).
pub struct HitSize(pub Mutex<(f64, f64, f64, f64)>);

/// Set from the tray menu.
pub struct UserHidden(pub Arc<AtomicBool>);

/// Whether the cursor is over the island, as last emitted in `island-hover`.
pub struct Hovered(pub AtomicBool);

#[tauri::command]
pub fn set_hit_size(state: State<HitSize>, x: f64, y: f64, width: f64, height: f64) {
    *state.0.lock().unwrap() = (x, y, width, height);
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

/// Dock the window where it was last left, on the primary monitor.
pub fn place(window: &WebviewWindow) -> tauri::Result<()> {
    let saved = config(window.app_handle()).and_then(|f| std::fs::read_to_string(f).ok());
    if let Some(d) = Dock::ALL.into_iter().find(|d| saved.as_deref().map(str::trim) == Some(d.name())) {
        DOCK.store(d as u8, Ordering::Relaxed);
    }
    if let Some(monitor) = window.primary_monitor()?.or(window.current_monitor()?) {
        layout(window, &monitor, dock())?;
    }
    window.set_ignore_cursor_events(true)
}

fn config(app: &AppHandle) -> Option<std::path::PathBuf> {
    Some(app.path().app_config_dir().ok()?.join(DOCK_FILE))
}

/// Size and place the window for `dock` on `monitor`: flush against that edge, and on a side
/// as tall as the space above the taskbar.
fn layout(window: &WebviewWindow, monitor: &tauri::Monitor, dock: Dock) -> tauri::Result<()> {
    let scale = monitor.scale_factor();
    let (mx, my) = (monitor.position().x, monitor.position().y);
    let mw = monitor.size().width as i32;
    let work = monitor.work_area();
    let (w, h, x, y) = match dock {
        Dock::Top => {
            let w = (WINDOW_WIDTH * scale).round() as i32;
            (w, (WINDOW_HEIGHT * scale).round() as i32, mx + (mw - w) / 2, my)
        }
        Dock::Left | Dock::Right => {
            let w = (SIDE_WIDTH * scale).round() as i32;
            let x = if dock == Dock::Left { mx } else { mx + mw - w };
            (w, work.size.height as i32, x, work.position.y)
        }
    };
    window.set_size(tauri::PhysicalSize::new(w, h))?;
    window.set_position(tauri::PhysicalPosition::new(x, y))
}

#[tauri::command]
pub fn island_dock() -> Dock {
    dock()
}

/// The edge of the cursor's monitor nearest the cursor.
fn nearest(app: &AppHandle, (cx, cy): (i32, i32)) -> Option<(tauri::Monitor, Dock)> {
    let monitor = app.monitor_from_point(cx as f64, cy as f64).ok().flatten()?;
    let (mx, my) = (monitor.position().x, monitor.position().y);
    let mw = monitor.size().width as i32;
    let edges = [(Dock::Top, cy - my), (Dock::Left, cx - mx), (Dock::Right, mx + mw - cx)];
    let dock = edges.into_iter().min_by_key(|&(_, d)| d).map(|(d, _)| d)?;
    Some((monitor, dock))
}

/// Carry the island with the cursor until the button is let go, then dock it at the nearest
/// edge. Windows' own window drag isn't used: it shows the window's outline and stalls the page.
#[tauri::command]
pub fn island_drag(app: AppHandle, window: WebviewWindow) {
    if DRAGGING.swap(true, Ordering::AcqRel) {
        return;
    }
    std::thread::spawn(move || {
        let _ = app.emit("island-dragging", true);
        let start = cursor_pos();
        let (left, top) = frame().map_or((0, 0), |(l, t, _, _)| (l, t));
        while button_down() {
            let now = cursor_pos();
            move_to(left + now.0 - start.0, top + now.1 - start.1);
            std::thread::sleep(DRAG_POLL);
        }
        if let Some((monitor, d)) = nearest(&app, cursor_pos()) {
            DOCK.store(d as u8, Ordering::Relaxed);
            if let Some(file) = config(&app) {
                let _ = std::fs::create_dir_all(file.parent().unwrap());
                let _ = std::fs::write(file, d.name());
            }
            let _ = layout(&window, &monitor, d);
            let _ = app.emit("island-dock", d);
        }
        let _ = app.emit("island-dragging", false);
        DRAGGING.store(false, Ordering::Release);
    });
}

/// Just under the island where it rests (beside it on a side), for the pet to hop out of and
/// climb back into.
pub(crate) fn pet_home(dog_w: f64, dog_h: f64) -> Option<(f64, f64)> {
    let (left, top, width, scale) = frame()?;
    let (l, t, w) = (left as f64, top as f64, width as f64);
    let middle = t + window_height()? / 2.0 + dog_h / 2.0 * scale;
    Some(match dock() {
        Dock::Top => (l + w / 2.0, t + (30.0 + dog_h) * scale),
        Dock::Left => (l + (50.0 + dog_w / 2.0) * scale, middle),
        Dock::Right => (l + w - (50.0 + dog_w / 2.0) * scale, middle),
    })
}

pub fn spawn_watcher(app: AppHandle, window: WebviewWindow) {
    let _ = HWND.set(window.hwnd().map_or(0, |h| h.0 as isize));
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

            let inside = cursor_inside(&app);
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

static HWND: std::sync::OnceLock<isize> = std::sync::OnceLock::new();

/// The island window's physical (left, top, width) and scale, straight from Windows: Tauri's
/// getters wait on the UI thread, and these are read many times a second. None while hidden.
#[cfg(target_os = "windows")]
pub(crate) fn frame() -> Option<(i32, i32, i32, f64)> {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowRect, IsWindowVisible};

    let hwnd = *HWND.get()? as windows_sys::Win32::Foundation::HWND;
    let mut r: RECT = unsafe { std::mem::zeroed() };
    if unsafe { IsWindowVisible(hwnd) == 0 || GetWindowRect(hwnd, &mut r) == 0 } {
        return None;
    }
    let scale = unsafe { GetDpiForWindow(hwnd) } as f64 / 96.0;
    Some((r.left, r.top, r.right - r.left, scale))
}

fn cursor_inside(app: &AppHandle) -> bool {
    let Some((left, top, _, scale)) = frame() else { return false };
    let (x, y, w, h) = *app.state::<HitSize>().0.lock().unwrap();
    let (cx, cy) = cursor_pos();
    let (cx, cy) = ((cx - left) as f64 / scale, (cy - top) as f64 / scale);
    cx >= x && cx <= x + w && cy >= y && cy <= y + h
}

/// The window's physical height, straight from Windows.
#[cfg(target_os = "windows")]
fn window_height() -> Option<f64> {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;
    let hwnd = *HWND.get()? as windows_sys::Win32::Foundation::HWND;
    let mut r: RECT = unsafe { std::mem::zeroed() };
    (unsafe { GetWindowRect(hwnd, &mut r) } != 0).then(|| (r.bottom - r.top) as f64)
}

/// Moves the window without waiting for the UI thread, so a drag never stutters.
#[cfg(target_os = "windows")]
fn move_to(x: i32, y: i32) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
    };
    let Some(&hwnd) = HWND.get() else { return };
    let flags = SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS;
    unsafe { SetWindowPos(hwnd as _, std::ptr::null_mut(), x, y, 0, 0, flags) };
}

/// The primary mouse button (the right one if they're swapped) is held.
#[cfg(target_os = "windows")]
fn button_down() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_SWAPBUTTON};
    let key = if unsafe { GetSystemMetrics(SM_SWAPBUTTON) } != 0 { VK_RBUTTON } else { VK_LBUTTON };
    unsafe { GetAsyncKeyState(key as i32) as u16 & 0x8000 != 0 }
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
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId,
    };

    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_null() {
            return false;
        }
        // Our own windows: circle to ask covers the whole screen (and Windows calls that
        // full-screen too), but it's no game.
        let mut pid = 0;
        GetWindowThreadProcessId(fg, &mut pid);
        if pid == GetCurrentProcessId() {
            return false;
        }

        let mut state = 0;
        if SHQueryUserNotificationState(&mut state) == 0
            && matches!(
                state,
                QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE
            )
        {
            return true;
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
fn window_height() -> Option<f64> {
    None
}

#[cfg(not(target_os = "windows"))]
fn move_to(_x: i32, _y: i32) {}

#[cfg(not(target_os = "windows"))]
fn button_down() -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
fn caps_lock_on() -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn fullscreen_app_active() -> bool {
    false
}
