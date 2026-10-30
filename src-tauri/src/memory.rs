//! What Choco knows about the user: short facts ("Uses Spotify, not YouTube, for music"), one per
//! line in `choco_memory.txt` in the app's config folder. Both models see them and can add to
//! them (`remember`) or drop one (`forget`); the user can also edit the file by hand.

use tauri::{AppHandle, Manager};

/// Oldest facts drop off past this, so the prompt stays small.
const MAX_FACTS: usize = 60;

fn file(app: &AppHandle) -> Option<std::path::PathBuf> {
    Some(app.path().app_config_dir().ok()?.join("choco_memory.txt"))
}

pub fn load(app: &AppHandle) -> Vec<String> {
    let Some(text) = file(app).and_then(|f| std::fs::read_to_string(f).ok()) else {
        return Vec::new();
    };
    text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()
}

fn save(app: &AppHandle, facts: &[String]) {
    let Some(f) = file(app) else { return };
    let _ = std::fs::create_dir_all(f.parent().unwrap());
    let _ = std::fs::write(f, facts.join("\n") + "\n");
}

/// For a system prompt: the facts as a list, or nothing yet.
pub fn prompt(app: &AppHandle) -> String {
    let facts = load(app);
    if facts.is_empty() {
        return "You don't know anything about the user yet.".into();
    }
    format!("What you know about the user:\n- {}", facts.join("\n- "))
}

pub fn remember(app: &AppHandle, fact: &str) {
    let fact = fact.trim().replace('\n', " ");
    if fact.is_empty() {
        return;
    }
    let mut facts = load(app);
    facts.retain(|f| !f.eq_ignore_ascii_case(&fact));
    facts.push(fact);
    let over = facts.len().saturating_sub(MAX_FACTS);
    facts.drain(..over);
    save(app, &facts);
}

/// Drops the fact that matches `fact` (or contains it).
pub fn forget(app: &AppHandle, fact: &str) {
    let want = fact.trim().to_lowercase();
    if want.is_empty() {
        return;
    }
    let mut facts = load(app);
    let before = facts.len();
    facts.retain(|f| f.to_lowercase() != want);
    if facts.len() == before {
        facts.retain(|f| !f.to_lowercase().contains(&want));
    }
    save(app, &facts);
}

/// The text both models get for the remember tool.
pub const REMEMBER: &str = "Save one short, lasting fact about the user, e.g. their name, the apps, \
sites and accounts they use, what they like. Use it when they tell you something about themselves \
or ask you to remember, and when you notice a lasting preference. Never passwords, codes or \
payment details.";
pub const FORGET: &str = "Drop a fact you know about the user, when they ask you to forget it or it \
turns out wrong. fact is the fact as you know it.";
