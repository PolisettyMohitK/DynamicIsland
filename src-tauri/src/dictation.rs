//! Dictation: hold Right Ctrl anywhere and talk; let go and the words are typed where the
//! cursor is. Moonshine turns speech into text on this machine; if Choco has an API key, a
//! quick model pass tidies it ("um", "nine… no, ten", punctuation). Off in game mode.
//!
//! Events: `dictation` {state: "listening" | "thinking" | "idle"}, {state: "typed", text}
//! or {state: "error", message}; `dictation-level` 0–1 while listening.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::ai::{Job, Provider};

/// Held this long with nothing else pressed before it counts as dictation, so
/// Right Ctrl shortcuts (Ctrl+C) never start it.
const HOLD: Duration = Duration::from_millis(300);
const MAX: Duration = Duration::from_secs(60);
/// Shorter than this is a tap, not speech.
const MIN_AUDIO: usize = 16_000 / 3;
const TIDY_TIMEOUT: Duration = Duration::from_secs(6);

static ENABLED: AtomicBool = AtomicBool::new(true);

const TIDY: &str = "You clean up dictated text from speech-to-text. Remove filler words (um, uh, \
you know), keep only the corrected version when the speaker corrects themselves (\"nine, no, ten\" \
becomes \"ten\"), fix obvious misheard words, punctuation and capitals. Keep their wording, tone and \
language; never add, answer, explain or summarise. Reply with the cleaned text only.";

#[derive(Serialize, Clone)]
#[serde(tag = "state", rename_all = "camelCase")]
enum Event {
    Listening,
    Thinking,
    Idle,
    Typed { text: String },
    Error { message: String },
}

fn emit(app: &AppHandle, e: Event) {
    let _ = app.emit("dictation", e);
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || run(app));
}

#[tauri::command]
pub fn dictation_set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

fn run(app: AppHandle) {
    loop {
        // A key poll every 40ms costs next to nothing; a keyboard hook would sit in
        // every key press system-wide, and anti-cheat software dislikes those.
        std::thread::sleep(Duration::from_millis(40));
        if !ENABLED.load(Ordering::Relaxed) || crate::game::on() || !keys::trigger_down() {
            continue;
        }
        let held = Instant::now();
        let mut alone = true;
        while keys::trigger_down() && held.elapsed() < HOLD {
            alone &= !keys::other_down();
            std::thread::sleep(Duration::from_millis(15));
        }
        if alone && keys::trigger_down() {
            match dictate(&app) {
                Ok(Some(text)) => emit(&app, Event::Typed { text }),
                Ok(None) => emit(&app, Event::Idle),
                Err(e) => emit(&app, Event::Error { message: e }),
            }
        }
        while keys::trigger_down() {
            std::thread::sleep(Duration::from_millis(40));
        }
    }
}

/// Records while the key is held, then types what was said. None if nothing was.
fn dictate(app: &AppHandle) -> Result<Option<String>, String> {
    let (stream, rx) = crate::choco::open_mic(crate::choco::mic(app).as_deref()).map_err(|e| format!("Microphone: {e}"))?;
    emit(app, Event::Listening);
    // Start loading the speech model now if "Hey Choco" hasn't, so it's ready on release.
    let app2 = app.clone();
    let loading = std::thread::spawn(move || crate::choco::load_stt(&app2).map_err(|e| e.to_string()));

    let mut audio = Vec::new();
    let started = Instant::now();
    while keys::trigger_down() && started.elapsed() < MAX {
        // Any other key means this wasn't dictation after all (or Esc to cancel).
        if keys::other_down() {
            drop(stream);
            return Ok(None);
        }
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(chunk) => {
                let rms = (chunk.iter().map(|v| v * v).sum::<f32>() / chunk.len().max(1) as f32).sqrt();
                let _ = app.emit("dictation-level", (rms / 3000.0).min(1.0));
                audio.extend(chunk);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Err("Microphone stopped".into()),
        }
    }
    drop(stream);
    if audio.len() < MIN_AUDIO {
        return Ok(None);
    }

    emit(app, Event::Thinking);
    loading.join().map_err(|_| "speech model crashed")?.map_err(|e| format!("Couldn't load the speech model: {e}"))?;
    let raw = crate::choco::transcribe(&audio).map_err(|e| e.to_string())?;
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    let text = tauri::async_runtime::block_on(tidy(app, raw)).unwrap_or_else(|| raw.to_string());
    keys::type_text(&text);
    Ok(Some(text))
}

/// The text cleaned up by the model, or None (no key, slow, offline) to use it as heard.
async fn tidy(app: &AppHandle, raw: &str) -> Option<String> {
    let (provider, key) = crate::ai::chosen(app)?;
    let body = json!({
        "model": provider.model(app, Job::Chat),
        "max_tokens": 1024,
        "messages": [
            { "role": "system", "content": TIDY },
            { "role": "user", "content": raw },
        ],
    });
    let mut req = reqwest::Client::new()
        .post(provider.chat_url())
        .bearer_auth(key)
        .header("user-agent", concat!("dynamic-island-choco/", env!("CARGO_PKG_VERSION")));
    if provider == Provider::Opencode {
        // Go routes by conversation; each dictation is its own.
        let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
        req = req.header("x-opencode-session", format!("choco-dictation-{ms}"));
    }
    let res = req.timeout(TIDY_TIMEOUT).json(&body).send().await.ok()?;
    let msg: serde_json::Value = res.error_for_status().ok()?.json().await.ok()?;
    let text = msg["choices"][0]["message"]["content"].as_str()?.trim();
    (!text.is_empty()).then(|| text.to_string())
}

#[cfg(target_os = "windows")]
mod keys {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

    const TRIGGER: u16 = VK_RCONTROL;

    fn down(vk: u16) -> bool {
        unsafe { GetAsyncKeyState(vk as i32) < 0 }
    }

    pub fn trigger_down() -> bool {
        down(TRIGGER)
    }

    /// Any key or mouse button besides Ctrl itself.
    pub fn other_down() -> bool {
        (1..=0xFEu16).any(|vk| vk != TRIGGER && vk != VK_CONTROL && down(vk))
    }

    /// Types `text` at the cursor of whatever app has focus (the island never does).
    pub fn type_text(text: &str) {
        let mut inputs = Vec::new();
        for unit in text.encode_utf16() {
            for flags in [KEYEVENTF_UNICODE, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP] {
                let mut i: INPUT = unsafe { std::mem::zeroed() };
                i.r#type = INPUT_KEYBOARD;
                i.Anonymous.ki = KEYBDINPUT { wVk: 0, wScan: unit, dwFlags: flags, time: 0, dwExtraInfo: 0 };
                inputs.push(i);
            }
        }
        unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
    }
}

#[cfg(not(target_os = "windows"))]
mod keys {
    pub fn trigger_down() -> bool {
        false
    }
    pub fn other_down() -> bool {
        false
    }
    pub fn type_text(_: &str) {}
}
