// Control Centre > Wallpaper: the person's own Pictures\Wallpapers folder, then the ones
// Windows ships. Thumbnails are shrunk here, so the webview never loads a full-size image.
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

const EXTS: [&str; 5] = ["jpg", "jpeg", "png", "bmp", "webp"];
const MAX: usize = 48;

#[derive(Serialize)]
pub struct Wall {
    pub path: String,
    pub current: bool,
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn wallpaper_list() -> Result<Vec<Wall>, String> {
    blocking(|| {
        let current = imp::current().to_lowercase();
        let mut files = Vec::new();
        if let Some(own) = imp::own_folder() {
            images(&own, 1, &mut files);
        }
        images(Path::new("C:\\Windows\\Web\\Wallpaper"), 1, &mut files);
        files.truncate(MAX);
        Ok(files
            .into_iter()
            .map(|p| {
                let path = p.to_string_lossy().to_string();
                Wall { current: path.to_lowercase() == current, path }
            })
            .collect())
    })
    .await
}

/// A small JPEG/PNG of the image as a data URL.
#[tauri::command]
pub async fn wallpaper_thumb(path: String) -> Result<String, String> {
    blocking(move || {
        // Kept for the run (at most MAX small PNGs, a few MB) so the tab opens filled.
        // The file's modified time is in the key, so an edited image gets a new thumbnail.
        let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        let key = (path.clone(), modified);
        if let Some(t) = THUMBS.lock().unwrap().get(&key) {
            return Ok(t.clone());
        }
        let thumb = imp::thumb(&path)?;
        THUMBS.lock().unwrap().insert(key, thumb.clone());
        Ok(thumb)
    })
    .await
}

static THUMBS: LazyLock<Mutex<HashMap<(String, Option<SystemTime>), String>>> = LazyLock::new(Default::default);

#[tauri::command]
pub async fn wallpaper_set(path: String) -> Result<(), String> {
    blocking(move || imp::set(&path)).await
}

/// Make Pictures\Wallpapers if needed and open it, so the person can drop images in.
#[tauri::command]
pub fn wallpaper_open_folder() -> Result<(), String> {
    let dir = imp::own_folder().ok_or("no Pictures folder")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::process::Command::new("explorer.exe").arg(&dir).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Image files in `dir`, and in its folders `depth` levels down.
fn images(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            if depth > 0 {
                images(&p, depth - 1, out);
            }
        } else if p.extension().and_then(|e| e.to_str()).is_some_and(|e| EXTS.contains(&e.to_lowercase().as_str())) {
            out.push(p);
        }
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use base64::Engine;
    use std::path::PathBuf;
    use windows::core::HSTRING;
    use windows::Win32::Foundation::GENERIC_READ;
    use windows::Win32::Graphics::Imaging::*;
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
    use windows_sys::Win32::UI::Shell::{FOLDERID_Pictures, SHGetKnownFolderPath};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_GETDESKWALLPAPER, SPI_SETDESKWALLPAPER,
    };

    const THUMB: u32 = 240;

    /// Pictures\Wallpapers, wherever Pictures lives (often OneDrive).
    pub fn own_folder() -> Option<PathBuf> {
        unsafe {
            let mut p = std::ptr::null_mut();
            if SHGetKnownFolderPath(&FOLDERID_Pictures, 0, std::ptr::null_mut(), &mut p) != 0 {
                return None;
            }
            let len = (0..).take_while(|&i| *p.add(i) != 0).count();
            let path = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
            CoTaskMemFree(Some(p as *const _));
            Some(PathBuf::from(path).join("Wallpapers"))
        }
    }

    pub fn current() -> String {
        let mut buf = [0u16; 520];
        unsafe { SystemParametersInfoW(SPI_GETDESKWALLPAPER, buf.len() as u32, buf.as_mut_ptr().cast(), 0) };
        let len = buf.iter().position(|&c| c == 0).unwrap_or(0);
        String::from_utf16_lossy(&buf[..len])
    }

    /// Decode and shrink with WIC, then encode as PNG. (The shell's thumbnail API gave
    /// back a generic file icon for these.)
    pub fn thumb(path: &str) -> Result<String, String> {
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        let run = || -> windows::core::Result<(u32, u32, Vec<u8>)> {
            unsafe {
                let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
                let decoder =
                    wic.CreateDecoderFromFilename(&HSTRING::from(path), None, GENERIC_READ, WICDecodeMetadataCacheOnDemand)?;
                let frame = decoder.GetFrame(0)?;
                let (mut w, mut h) = (0, 0);
                frame.GetSize(&mut w, &mut h)?;
                let rgba = wic.CreateFormatConverter()?;
                rgba.Initialize(&frame, &GUID_WICPixelFormat32bppRGBA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom)?;
                let tw = THUMB.min(w.max(1));
                let th = (h as u64 * tw as u64 / w.max(1) as u64).max(1) as u32;
                let scaler = wic.CreateBitmapScaler()?;
                scaler.Initialize(&rgba, tw, th, WICBitmapInterpolationModeFant)?;
                let mut pixels = vec![0u8; (tw * th * 4) as usize];
                scaler.CopyPixels(std::ptr::null(), tw * 4, &mut pixels)?;
                Ok((tw, th, pixels))
            }
        };
        let (w, h, pixels) = run().map_err(|e| format!("thumbnail: {e}"))?;
        let mut bytes = Vec::new();
        let mut enc = png::Encoder::new(&mut bytes, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()
            .and_then(|mut wr| wr.write_image_data(&pixels))
            .map_err(|e| format!("thumbnail: {e}"))?;
        Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&bytes)))
    }

    pub fn set(path: &str) -> Result<(), String> {
        let wide: Vec<u16> = path.encode_utf16().chain([0]).collect();
        let ok = unsafe {
            SystemParametersInfoW(SPI_SETDESKWALLPAPER, 0, wide.as_ptr() as *mut _, SPIF_UPDATEINIFILE | SPIF_SENDCHANGE)
        };
        if ok == 0 {
            Err("Windows didn't take that wallpaper".into())
        } else {
            Ok(())
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use std::path::PathBuf;
    pub fn own_folder() -> Option<PathBuf> {
        None
    }
    pub fn current() -> String {
        String::new()
    }
    pub fn thumb(_: &str) -> Result<String, String> {
        Err("wallpaper only works on Windows".into())
    }
    pub fn set(_: &str) -> Result<(), String> {
        Err("wallpaper only works on Windows".into())
    }
}
