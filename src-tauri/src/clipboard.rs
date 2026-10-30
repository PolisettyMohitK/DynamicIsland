// Control Centre > Clipboard: the last few things copied, kept in memory only.
// A hidden message window hears every clipboard change; text is kept unless the
// app that copied it asked to stay out of clipboard history (password managers do).
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter};

const KEEP: usize = 30;
static HISTORY: Mutex<Vec<String>> = Mutex::new(Vec::new());
static APP: OnceLock<AppHandle> = OnceLock::new();

pub fn start(app: AppHandle) {
    if APP.set(app).is_ok() {
        std::thread::spawn(win::listen);
    }
}

#[tauri::command]
pub fn clipboard_history() -> Vec<String> {
    HISTORY.lock().unwrap().clone()
}

#[tauri::command]
pub fn clipboard_clear() {
    HISTORY.lock().unwrap().clear();
}

/// Put `text` back on the clipboard and paste it into whatever app has focus.
/// The island never takes focus, so that's the app the person was just using.
#[tauri::command]
pub async fn clipboard_paste(text: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || win::paste(&text)).await.map_err(|e| e.to_string())?
}

fn remember(text: String) {
    if text.trim().is_empty() {
        return;
    }
    let mut h = HISTORY.lock().unwrap();
    h.retain(|t| *t != text);
    h.insert(0, text);
    h.truncate(KEEP);
    drop(h);
    if let Some(app) = APP.get() {
        let _ = app.emit("clipboard-changed", ());
    }
}

#[cfg(target_os = "windows")]
mod win {
    use std::ptr::null_mut;
    use windows_sys::w;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::DataExchange::{
        AddClipboardFormatListener, CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable,
        OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    const CF_UNICODETEXT: u32 = 13;

    pub fn listen() {
        unsafe {
            let class = WNDCLASSW { lpfnWndProc: Some(proc), lpszClassName: w!("IslandClipboard"), ..std::mem::zeroed() };
            RegisterClassW(&class);
            let hwnd = CreateWindowExW(0, w!("IslandClipboard"), w!(""), 0, 0, 0, 0, 0, HWND_MESSAGE, null_mut(), null_mut(), null_mut());
            if hwnd.is_null() || AddClipboardFormatListener(hwnd) == 0 {
                return;
            }
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                DispatchMessageW(&msg);
            }
        }
    }

    unsafe extern "system" fn proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        if msg == WM_CLIPBOARDUPDATE {
            if let Some(text) = read() {
                super::remember(text);
            }
            return 0;
        }
        DefWindowProcW(hwnd, msg, w, l)
    }

    /// The clipboard's text, unless its owner marked it private.
    unsafe fn read() -> Option<String> {
        if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
            return None;
        }
        let exclude = RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
        let history = RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory"));
        // Whoever just copied may still hold it open; a few tries is enough.
        let mut open = false;
        for _ in 0..5 {
            if OpenClipboard(null_mut()) != 0 {
                open = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if !open {
            return None;
        }
        let private = IsClipboardFormatAvailable(exclude) != 0 || {
            let h = GetClipboardData(history);
            !h.is_null() && {
                let p = GlobalLock(h) as *const u32;
                let allowed = p.is_null() || *p != 0;
                GlobalUnlock(h);
                !allowed
            }
        };
        let mut text = None;
        if !private {
            let h = GetClipboardData(CF_UNICODETEXT);
            let p = if h.is_null() { null_mut() } else { GlobalLock(h) as *const u16 };
            if !p.is_null() {
                let len = (0..).take_while(|&i| *p.add(i) != 0).count();
                text = Some(String::from_utf16_lossy(std::slice::from_raw_parts(p, len)));
                GlobalUnlock(h);
            }
        }
        CloseClipboard();
        text
    }

    pub fn paste(text: &str) -> Result<(), String> {
        unsafe {
            let wide: Vec<u16> = text.encode_utf16().chain([0]).collect();
            let h = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2);
            if h.is_null() {
                return Err("out of memory".into());
            }
            let p = GlobalLock(h) as *mut u16;
            std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
            GlobalUnlock(h);
            if OpenClipboard(null_mut()) == 0 {
                return Err("the clipboard is busy".into());
            }
            EmptyClipboard();
            // The clipboard owns the memory once this succeeds.
            SetClipboardData(CF_UNICODETEXT, h);
            CloseClipboard();

            let key = |vk: u16, up: bool| {
                let mut i: INPUT = std::mem::zeroed();
                i.r#type = INPUT_KEYBOARD;
                i.Anonymous.ki.wVk = vk;
                i.Anonymous.ki.dwFlags = if up { KEYEVENTF_KEYUP } else { 0 };
                i
            };
            let keys = [key(VK_CONTROL, false), key(b'V' as u16, false), key(b'V' as u16, true), key(VK_CONTROL, true)];
            SendInput(keys.len() as u32, keys.as_ptr(), std::mem::size_of::<INPUT>() as i32);
        }
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
mod win {
    pub fn listen() {}
    pub fn paste(_: &str) -> Result<(), String> {
        Err("clipboard paste only works on Windows".into())
    }
}
