// File shelf: thumbnails, opening, and dragging files back out of the island.
use std::path::PathBuf;
use tauri::ipc::Response;
use tauri::WebviewWindow;

const IMAGE_EXTS: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "bmp"];
const MAX_THUMB_BYTES: u64 = 30 * 1024 * 1024;

/// The file's own bytes when it's an image small enough to show; empty otherwise.
#[tauri::command]
pub async fn shelf_thumb(path: String) -> Response {
    let path = PathBuf::from(path);
    let is_image = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_EXTS.contains(&e.to_ascii_lowercase().as_str()));
    let small = std::fs::metadata(&path).is_ok_and(|m| m.len() <= MAX_THUMB_BYTES);
    let bytes = if is_image && small { std::fs::read(&path).unwrap_or_default() } else { Vec::new() };
    Response::new(bytes)
}

#[tauri::command]
pub fn shelf_exists(paths: Vec<String>) -> Vec<bool> {
    paths.iter().map(|p| std::path::Path::new(p).exists()).collect()
}

#[tauri::command]
pub fn shelf_open(path: String) -> Result<(), String> {
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|e| e.to_string())
}

/// Start a native drag of these files, so they can be dropped into Explorer, a chat, an editor...
#[tauri::command]
pub fn shelf_drag(window: WebviewWindow, paths: Vec<String>) -> Result<(), String> {
    let files: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).filter(|p| p.exists()).collect();
    if files.is_empty() {
        return Err("files no longer exist".into());
    }
    let target = window.clone();
    window
        .run_on_main_thread(move || {
            let icon = drag::Image::Raw(include_bytes!("../icons/32x32.png").to_vec());
            if let Err(e) = drag::start_drag(&target, drag::DragItem::Files(files), icon, |_, _| {}, Default::default())
            {
                eprintln!("drag out failed: {e}");
            }
        })
        .map_err(|e| e.to_string())
}
