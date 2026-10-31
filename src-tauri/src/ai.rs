//! Which AI service Choco thinks with, and its keys. OpenCode (Go plan), OpenAI or Google
//! Gemini; each key is kept in its own file in the app's config folder (or comes from an
//! environment variable), never in the repo. The chosen service answers questions (ask.rs)
//! and works the desktop (computer.rs).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Opencode,
    Openai,
    Gemini,
}

/// What a model is used for.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Job {
    /// Answering questions.
    Chat,
    /// Working the desktop.
    Computer,
}

impl Job {
    fn name(self) -> &'static str {
        match self {
            Job::Chat => "chat",
            Job::Computer => "computer",
        }
    }
}

const ALL: [Provider; 3] = [Provider::Opencode, Provider::Openai, Provider::Gemini];
/// Which service is in use: "opencode", "openai" or "gemini".
const CHOICE_FILE: &str = "ai_provider.txt";
/// Models you picked: {"openai": {"chat": "...", "computer": "..."}, ...}.
const MODELS_FILE: &str = "ai_models.json";

impl Provider {
    fn name(self) -> &'static str {
        match self {
            Provider::Opencode => "opencode",
            Provider::Openai => "openai",
            Provider::Gemini => "gemini",
        }
    }

    fn env(self) -> &'static str {
        match self {
            Provider::Opencode => "OPENCODE_API_KEY",
            Provider::Openai => "OPENAI_API_KEY",
            Provider::Gemini => "GEMINI_API_KEY",
        }
    }

    /// OpenAI-style chat completions, for questions (all three) and computer use (not OpenCode).
    pub fn chat_url(self) -> &'static str {
        match self {
            Provider::Opencode => "https://opencode.ai/inference/go/openai/v1/chat/completions",
            Provider::Openai => "https://api.openai.com/v1/chat/completions",
            Provider::Gemini => "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        }
    }

    /// The model used when you haven't picked one.
    fn default_model(self, job: Job) -> &'static str {
        match (self, job) {
            // Questions: quick and cheap.
            (Provider::Opencode, Job::Chat) => "deepseek-v4.1-flash",
            (Provider::Openai, Job::Chat) => "gpt-6-luna",
            (Provider::Gemini, Job::Chat) => "gemini-3.8-flash",
            // Working the desktop from screenshots. OpenCode's goes to Claude (computer.rs).
            (Provider::Opencode, Job::Computer) => "claude-haiku-5-5",
            (Provider::Openai, Job::Computer) => "gpt-6.1-terra",
            (Provider::Gemini, Job::Computer) => "gemini-3.8-flash",
        }
    }

    /// Your pick for `job` (Settings), else the default.
    pub fn model(self, app: &AppHandle, job: Job) -> String {
        saved_models(app)[self.name()][job.name()]
            .as_str()
            .filter(|m| !m.trim().is_empty())
            .unwrap_or(self.default_model(job))
            .to_string()
    }

    fn key(self, app: &AppHandle) -> Option<String> {
        if let Ok(k) = std::env::var(self.env()) {
            if !k.trim().is_empty() {
                return Some(k.trim().to_string());
            }
        }
        let k = std::fs::read_to_string(config(app, &format!("{}_key.txt", self.name()))?).ok()?;
        (!k.trim().is_empty()).then(|| k.trim().to_string())
    }
}

fn config(app: &AppHandle, file: &str) -> Option<std::path::PathBuf> {
    Some(app.path().app_config_dir().ok()?.join(file))
}

fn write_config(app: &AppHandle, file: &str, text: &str) -> Result<(), String> {
    let file = config(app, file).ok_or("no config folder")?;
    std::fs::create_dir_all(file.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(file, text).map_err(|e| e.to_string())
}

fn saved_models(app: &AppHandle) -> Value {
    config(app, MODELS_FILE)
        .and_then(|f| std::fs::read(f).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| json!({}))
}

/// The chosen service; with no choice saved, the first one that has a key.
fn provider(app: &AppHandle) -> Provider {
    let saved = config(app, CHOICE_FILE).and_then(|f| std::fs::read_to_string(f).ok());
    ALL.into_iter()
        .find(|p| saved.as_deref().map(str::trim) == Some(p.name()))
        .or_else(|| ALL.into_iter().find(|p| p.key(app).is_some()))
        .unwrap_or(Provider::Opencode)
}

fn save_provider(app: &AppHandle, p: Provider) -> Result<(), String> {
    write_config(app, CHOICE_FILE, p.name())
}

/// The service to use and its key, or None when it has no key.
pub fn chosen(app: &AppHandle) -> Option<(Provider, String)> {
    let p = provider(app);
    Some((p, p.key(app)?))
}

/// For Settings: the service in use, and for each service whether it has a key and its
/// models: `{provider, services: {openai: {key, chat, computer, defaultChat, defaultComputer}, ...}}`.
#[tauri::command]
pub fn ai_status(app: AppHandle) -> Value {
    let mut services = json!({});
    for p in ALL {
        services[p.name()] = json!({
            "key": p.key(&app).is_some(),
            "chat": p.model(&app, Job::Chat),
            "computer": p.model(&app, Job::Computer),
            "defaultChat": p.default_model(Job::Chat),
            "defaultComputer": p.default_model(Job::Computer),
        });
    }
    json!({ "provider": provider(&app), "services": services })
}

#[tauri::command]
pub fn ai_set_provider(app: AppHandle, provider: Provider) -> Result<(), String> {
    save_provider(&app, provider)
}

/// Picks the model `provider` uses for `job`; an empty name goes back to the default.
#[tauri::command]
pub fn ai_set_model(app: AppHandle, provider: Provider, job: Job, model: String) -> Result<(), String> {
    let mut models = saved_models(&app);
    let model = model.trim();
    if model.is_empty() || model == provider.default_model(job) {
        if let Some(m) = models[provider.name()].as_object_mut() {
            m.remove(job.name());
        }
    } else {
        models[provider.name()][job.name()] = json!(model);
    }
    write_config(&app, MODELS_FILE, &models.to_string())
}

/// Model ids that aren't for talking to: embeddings, speech, images, video and the like.
const NOT_CHAT: [&str; 14] = [
    "embed", "tts", "whisper", "dall-e", "image", "audio", "moderation", "realtime", "transcribe", "search",
    "imagen", "veo", "aqa", "sora",
];

/// For the model dropdowns: the models `provider`'s key can use, from the service's own list.
/// Computer use with OpenCode goes to Claude (computer.rs), so only Claude models are offered
/// for it. Empty without a key or when the service won't say.
#[tauri::command]
pub async fn ai_models(app: AppHandle, provider: Provider, job: Job) -> Vec<String> {
    let Some(key) = provider.key(&app) else { return Vec::new() };
    let url = provider.chat_url().replace("/chat/completions", "/models");
    let body: Value = match reqwest::Client::new().get(url).bearer_auth(key).send().await {
        Ok(r) if r.status().is_success() => r.json().await.unwrap_or_default(),
        _ => return Vec::new(),
    };
    let mut ids: Vec<String> = body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m["id"].as_str())
        // Gemini's ids come as "models/gemini-…".
        .map(|id| id.trim_start_matches("models/").to_string())
        .filter(|id| !NOT_CHAT.iter().any(|n| id.contains(n)))
        .filter(|id| !matches!((provider, job), (Provider::Opencode, Job::Computer)) || id.starts_with("claude"))
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// Saves a key; an empty key removes it. A first key also makes its service the one in use.
#[tauri::command]
pub fn ai_set_key(app: AppHandle, provider: Provider, key: String) -> Result<(), String> {
    let name = format!("{}_key.txt", provider.name());
    if key.trim().is_empty() {
        if let Some(file) = config(&app, &name) {
            let _ = std::fs::remove_file(file);
        }
        return Ok(());
    }
    let in_use = self::provider(&app);
    write_config(&app, &name, key.trim())?;
    if in_use != provider && in_use.key(&app).is_none() {
        save_provider(&app, provider)?;
    }
    Ok(())
}

/// The message from a failed response: OpenAI-style `{error}` or Gemini's `[{error}]`.
pub fn error_message(body: &Value) -> &str {
    body["error"]["message"].as_str().or(body[0]["error"]["message"].as_str()).unwrap_or("request failed")
}
