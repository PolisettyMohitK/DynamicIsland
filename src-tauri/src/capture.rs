// Captures the strip of screen behind the island so the frontend can draw it
// through the glass. The island window is excluded from capture, so it never
// sees itself.
use std::sync::Mutex;

use tauri::ipc::Response;
use tauri::{State, WebviewWindow};

use crate::island::{WINDOW_HEIGHT, WINDOW_WIDTH};

/// The last frame sent, so unchanged frames aren't sent again.
pub struct LastFrame(pub Mutex<Vec<u8>>);

/// Hide the island from every screen capture, including our own.
pub fn exclude_from_capture(window: &WebviewWindow) {
    #[cfg(target_os = "windows")]
    if let Ok(hwnd) = window.hwnd() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
        };
        unsafe { SetWindowDisplayAffinity(hwnd.0 as _, WDA_EXCLUDEFROMCAPTURE) };
    }
}

/// RGBA pixels of the top `height` logical px of the screen behind the window,
/// at logical resolution and `WINDOW_WIDTH` wide. Empty when the window is
/// hidden, or when nothing changed since the last call unless `force` is set.
#[tauri::command]
pub async fn capture_backdrop(
    last: State<'_, LastFrame>,
    height: f64,
    force: bool,
) -> Result<Response, String> {
    let Some((x, y, _, scale)) = crate::island::frame() else {
        return Ok(Response::new(Vec::new()));
    };
    let out_w = WINDOW_WIDTH as i32;
    let out_h = height.round().clamp(1.0, WINDOW_HEIGHT) as i32;
    let src_w = (WINDOW_WIDTH * scale).round() as i32;
    let src_h = (out_h as f64 * scale).round() as i32;

    let frame = grab(x, y, src_w, src_h, out_w, out_h);
    let mut last = last.0.lock().unwrap();
    if !force && *last == frame {
        return Ok(Response::new(Vec::new()));
    }
    last.clone_from(&frame);
    Ok(Response::new(frame))
}

#[cfg(target_os = "windows")]
pub(crate) fn grab(x: i32, y: i32, src_w: i32, src_h: i32, out_w: i32, out_h: i32) -> Vec<u8> {
    use std::ptr::null_mut;
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC,
        SelectObject, SetBrushOrgEx, SetStretchBltMode, StretchBlt, BITMAPINFO, BITMAPINFOHEADER,
        BI_RGB, DIB_RGB_COLORS, HALFTONE, SRCCOPY,
    };

    let len = (out_w * out_h * 4) as usize;
    let mut pixels = vec![0u8; len];
    unsafe {
        let screen = GetDC(null_mut());
        let mem = CreateCompatibleDC(screen);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: out_w,
            biHeight: -out_h, // top-down rows
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..std::mem::zeroed()
        };
        let mut bits = null_mut();
        let dib = CreateDIBSection(screen, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if !dib.is_null() && !bits.is_null() {
            let old = SelectObject(mem, dib);
            SetStretchBltMode(mem, HALFTONE);
            SetBrushOrgEx(mem, 0, 0, null_mut());
            StretchBlt(mem, 0, 0, out_w, out_h, screen, x, y, src_w, src_h, SRCCOPY);
            std::ptr::copy_nonoverlapping(bits as *const u8, pixels.as_mut_ptr(), len);
            SelectObject(mem, old);
            DeleteObject(dib);
        }
        DeleteDC(mem);
        ReleaseDC(null_mut(), screen);
    }
    // BGRA -> RGBA, opaque
    for px in pixels.chunks_exact_mut(4) {
        px.swap(0, 2);
        px[3] = 255;
    }
    pixels
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn grab(_x: i32, _y: i32, _src_w: i32, _src_h: i32, out_w: i32, out_h: i32) -> Vec<u8> {
    vec![0; (out_w * out_h * 4) as usize]
}
