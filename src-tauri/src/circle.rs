//! Circle to ask: Ctrl+Shift+Space dims the screen; draw around anything, then ask about it
//! out loud or by typing. The circled part goes to Claude Haiku 5.5 (through OpenCode Go,
//! like computer use) with the question, and the answer appears next to the circle.
//! The shortcut is off in game mode.
//!
//! Events to the overlay: `circle-open`, `circle-level` 0–1 while listening.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

const URL: &str = "https://opencode.ai/inference/go/anthropic/v1/messages";
const MODEL: &str = "claude-haiku-5-5";
/// The circled part goes out at most this many pixels on its long side.
const MAX_SIDE: i32 = 1024;
const SYSTEM: &str = "The user circled part of their Windows screen and asked about it. The image is \
what they circled. Answer like a knowledgeable friend: plain and direct, no filler. Usually two to \
four sentences; up to about 120 words when it needs steps or an explanation, with lines starting \
\"- \" for bullets. No other formatting. If the image doesn't show enough to answer, say what you \
can see and what's missing.";

/// Wait this long for the first word, then stop after this much quiet.
const NO_SPEECH: Duration = Duration::from_secs(6);
const END_SILENCE: Duration = Duration::from_millis(900);
const MAX_QUESTION: Duration = Duration::from_secs(15);

/// Set when the overlay closes or the person starts typing instead.
static STOP_LISTENING: AtomicBool = AtomicBool::new(false);

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut was_down = false;
        loop {
            std::thread::sleep(Duration::from_millis(50));
            if crate::game::on() {
                // Nothing may pop up over a game.
                if let Some(w) = app.get_webview_window("circle") {
                    let _ = w.hide();
                }
                was_down = false;
                continue;
            }
            let down = keys::chord_down();
            if down && !was_down {
                if let Err(e) = open(&app) {
                    eprintln!("[circle] {e}");
                }
            }
            was_down = down;
        }
    });
}

fn open(app: &AppHandle) -> tauri::Result<()> {
    let window = match app.get_webview_window("circle") {
        Some(w) => w,
        None => build(app)?,
    };
    // Cover the monitor the cursor is on.
    if let Some(m) = window.cursor_position().ok().and_then(|p| window.monitor_from_point(p.x, p.y).ok().flatten()) {
        window.set_position(*m.position())?;
        window.set_size(*m.size())?;
    }
    let _ = app.emit_to("circle", "circle-open", ());
    window.show()?;
    window.set_focus()?;
    Ok(())
}

fn build(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, "circle", WebviewUrl::App("circle".into()))
        .title("Circle to ask")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .visible(false)
        // Must match the main window's, or WebView2 refuses the second window.
        .additional_browser_args(
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --autoplay-policy=no-user-gesture-required",
        )
        .build()?;
    // The screenshot of the circled part must show the screen, not the dimmed overlay.
    crate::capture::exclude_from_capture(&window);
    Ok(window)
}

#[tauri::command]
pub fn circle_close(window: WebviewWindow) {
    STOP_LISTENING.store(true, Ordering::Relaxed);
    let _ = window.hide();
}

#[tauri::command]
pub fn circle_stop_listening() {
    STOP_LISTENING.store(true, Ordering::Relaxed);
}

/// Records the spoken question and returns it as text ("" if nothing was said).
#[tauri::command]
pub async fn circle_listen(app: AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || listen(&app)).await.map_err(|e| e.to_string())?
}

fn listen(app: &AppHandle) -> Result<String, String> {
    STOP_LISTENING.store(false, Ordering::Relaxed);
    let app2 = app.clone();
    let loading = std::thread::spawn(move || crate::choco::load_stt(&app2).map_err(|e| e.to_string()));
    let (stream, rx) = crate::choco::open_mic(crate::choco::mic(app).as_deref()).map_err(|e| format!("Microphone: {e}"))?;

    let mut audio = Vec::new();
    let started = Instant::now();
    let mut floor = 200.0f32;
    let mut last_voice: Option<Instant> = None;
    loop {
        if STOP_LISTENING.load(Ordering::Relaxed) {
            return Ok(String::new());
        }
        match last_voice {
            None if started.elapsed() > NO_SPEECH => return Ok(String::new()),
            Some(t) if t.elapsed() > END_SILENCE || started.elapsed() > MAX_QUESTION => break,
            _ => {}
        }
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(chunk) => {
                let level = (chunk.iter().map(|v| v * v).sum::<f32>() / chunk.len().max(1) as f32).sqrt();
                let _ = app.emit_to("circle", "circle-level", (level / 3000.0).min(1.0));
                if level > (floor * 2.5).max(300.0) {
                    last_voice = Some(Instant::now());
                } else if last_voice.is_none() {
                    floor = floor * 0.9 + level * 0.1;
                }
                audio.extend(chunk);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Err("Microphone stopped".into()),
        }
    }
    drop(stream);
    loading.join().map_err(|_| "speech model crashed")?.map_err(|e| format!("Couldn't load the speech model: {e}"))?;
    crate::choco::transcribe(&audio).map(|t| t.trim().to_string()).map_err(|e| e.to_string())
}

/// `x, y, w, h`: the circled area in physical pixels, relative to the overlay window.
#[tauri::command]
pub async fn circle_ask(app: AppHandle, window: WebviewWindow, x: i32, y: i32, w: i32, h: i32, question: String) -> Result<String, String> {
    let key = crate::ask::api_key(&app).ok_or("Add an OpenCode key in Choco's settings first.")?;
    let origin = window.outer_position().map_err(|e| e.to_string())?;
    let (w, h) = (w.max(8), h.max(8));
    let scale = (MAX_SIDE as f32 / w.max(h) as f32).min(1.0);
    let (ow, oh) = (((w as f32) * scale).round() as i32, ((h as f32) * scale).round() as i32);
    let png = tauri::async_runtime::spawn_blocking(move || {
        let rgba = crate::capture::grab(origin.x + x, origin.y + y, w, h, ow, oh);
        crate::computer::png_base64(&rgba, ow, oh)
    })
    .await
    .map_err(|e| e.to_string())?;

    let question = if question.trim().is_empty() { "What is this?".to_string() } else { question };
    let body = json!({
        "model": MODEL,
        "max_tokens": 1024,
        "system": SYSTEM,
        "messages": [{
            "role": "user",
            "content": [
                { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": png } },
                { "type": "text", "text": question },
            ],
        }],
    });
    let res = reqwest::Client::new()
        .post(URL)
        .header("x-api-key", key)
        // Go routes by conversation; each question is its own.
        .header("x-opencode-session", format!("choco-circle-{}", now_ms()))
        .header("anthropic-version", "2023-06-01")
        .header("user-agent", concat!("dynamic-island-choco/", env!("CARGO_PKG_VERSION")))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?;
    let status = res.status();
    let msg: serde_json::Value = res.json().await.map_err(|e| format!("bad response: {e}"))?;
    if !status.is_success() {
        let detail = msg["error"]["message"].as_str().unwrap_or("request failed");
        return Err(format!("{}: {detail}", status.as_u16()));
    }
    let text = msg["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join(" ");
    Ok(text.trim().to_string())
}

fn now_ms() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()
}

#[cfg(target_os = "windows")]
mod keys {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_SHIFT, VK_SPACE};

    pub fn chord_down() -> bool {
        let down = |vk: u16| unsafe { GetAsyncKeyState(vk as i32) < 0 };
        down(VK_CONTROL) && down(VK_SHIFT) && down(VK_SPACE)
    }
}

#[cfg(not(target_os = "windows"))]
mod keys {
    pub fn chord_down() -> bool {
        false
    }
}
