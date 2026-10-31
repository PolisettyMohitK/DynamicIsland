//! Circle to ask: Ctrl+Shift+Space dims the screen until you ask, or press it (or Esc) again.
//! Draw around anything, as often as you like, then ask about it out loud or by typing. The
//! circled part goes with the question to the computer-use model of the AI service in use
//! (ai.rs; Claude Haiku 5.5 through OpenCode Go); the screen clears and Choco answers on the
//! island. The shortcut is off in game mode.
//!
//! Events to the overlay: `circle-open`, `circle-level` 0–1 while listening.
//! Events to the island: `circle-asked` {question}, then `circle-answer` {question, reply}.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::ai::{Job, Provider};
use crate::computer::CLAUDE_URL;

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
    // The shortcut belongs to this thread: Windows posts it here as a message.
    std::thread::spawn(move || {
        let mut claimed = false;
        loop {
            std::thread::sleep(Duration::from_millis(50));
            if crate::game::on() {
                // Nothing may pop up over a game, and the keys are the game's.
                if let Some(w) = app.get_webview_window("circle") {
                    close(&w);
                }
                if claimed {
                    keys::release();
                    claimed = false;
                }
                continue;
            }
            if !claimed {
                // Fails while another app holds the same shortcut; keeps trying.
                claimed = keys::claim();
            }
            if claimed && keys::pressed() {
                match app.get_webview_window("circle") {
                    Some(w) if w.is_visible().unwrap_or(false) => close(&w),
                    _ => {
                        if let Err(e) = open(&app) {
                            eprintln!("[circle] {e}");
                        }
                    }
                }
            }
        }
    });
}

fn close(window: &WebviewWindow) {
    STOP_LISTENING.store(true, Ordering::Relaxed);
    let _ = window.hide();
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
    close(&window);
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

/// `x, y, w, h`: the circled area in physical pixels, relative to the overlay window. Takes the
/// picture, clears the screen and hands the question to the island, which shows the answer.
#[tauri::command]
pub async fn circle_ask(app: AppHandle, window: WebviewWindow, x: i32, y: i32, w: i32, h: i32, question: String) {
    let question = if question.trim().is_empty() { "What is this?".to_string() } else { question.trim().to_string() };
    let _ = app.emit("circle-asked", json!({ "question": question }));
    let reply = match answer(&app, &window, (x, y, w, h), &question).await {
        Ok(text) if text.is_empty() => json!({ "text": "No answer came back.", "tone": "bad" }),
        Ok(text) => json!({ "text": text }),
        Err(e) => json!({ "text": e, "tone": "bad" }),
    };
    let _ = app.emit("circle-answer", json!({ "question": question, "reply": reply }));
}

async fn answer(app: &AppHandle, window: &WebviewWindow, (x, y, w, h): (i32, i32, i32, i32), question: &str) -> Result<String, String> {
    let origin = window.outer_position().map_err(|e| e.to_string());
    close(window);
    let origin = origin?;
    let (provider, key) = crate::ai::chosen(app).ok_or("Add an AI key on the dashboard first.")?;
    let (w, h) = (w.max(8), h.max(8));
    let scale = (MAX_SIDE as f32 / w.max(h) as f32).min(1.0);
    let (ow, oh) = (((w as f32) * scale).round() as i32, ((h as f32) * scale).round() as i32);
    let png = tauri::async_runtime::spawn_blocking(move || {
        let rgba = crate::capture::grab(origin.x + x, origin.y + y, w, h, ow, oh);
        crate::computer::png_base64(&rgba, ow, oh)
    })
    .await
    .map_err(|e| e.to_string())?;

    let model = provider.model(app, Job::Computer);
    let client = reqwest::Client::new();
    let req = if provider == Provider::Opencode {
        client
            .post(CLAUDE_URL)
            .header("x-api-key", key)
            // Go routes by conversation; each question is its own.
            .header("x-opencode-session", format!("choco-circle-{}", now_ms()))
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": model,
                "max_tokens": 1024,
                "system": SYSTEM,
                "messages": [{
                    "role": "user",
                    "content": [
                        { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": png } },
                        { "type": "text", "text": question },
                    ],
                }],
            }))
    } else {
        client.post(provider.chat_url()).bearer_auth(key).json(&json!({
            "model": model,
            "max_tokens": 1024,
            "messages": [
                { "role": "system", "content": SYSTEM },
                {
                    "role": "user",
                    "content": [
                        { "type": "image_url", "image_url": { "url": format!("data:image/png;base64,{png}") } },
                        { "type": "text", "text": question },
                    ],
                },
            ],
        }))
    };
    let res = req
        .header("user-agent", concat!("dynamic-island-choco/", env!("CARGO_PKG_VERSION")))
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?;
    let status = res.status();
    let msg: serde_json::Value = res.json().await.map_err(|e| format!("bad response: {e}"))?;
    if !status.is_success() {
        return Err(format!("{}: {}", status.as_u16(), crate::ai::error_message(&msg)));
    }
    let text = if provider == Provider::Opencode {
        msg["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        msg["choices"][0]["message"]["content"].as_str().unwrap_or_default().to_string()
    };
    Ok(text.trim().to_string())
}

fn now_ms() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()
}

/// Ctrl+Shift+Space as a Windows hotkey: the press is ours alone (it doesn't also type a space
/// into the app underneath), and holding it down counts once.
#[cfg(target_os = "windows")]
mod keys {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        RegisterHotKey, UnregisterHotKey, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, VK_SPACE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{PeekMessageW, MSG, PM_REMOVE, WM_HOTKEY};

    const ID: i32 = 1;

    /// For the calling thread; false if another app already has the shortcut.
    pub fn claim() -> bool {
        unsafe { RegisterHotKey(std::ptr::null_mut(), ID, MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT, VK_SPACE as u32) != 0 }
    }

    pub fn release() {
        unsafe { UnregisterHotKey(std::ptr::null_mut(), ID) };
    }

    /// Whether it was pressed since the last call.
    pub fn pressed() -> bool {
        let mut hit = false;
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        while unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), WM_HOTKEY, WM_HOTKEY, PM_REMOVE) } != 0 {
            hit = true;
        }
        hit
    }
}

#[cfg(not(target_os = "windows"))]
mod keys {
    pub fn claim() -> bool {
        false
    }
    pub fn release() {}
    pub fn pressed() -> bool {
        false
    }
}
