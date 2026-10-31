// Claude Code in the island: your recent sessions and what each is up to, read from the
// transcripts Claude Code keeps in ~/.claude/projects (the desktop app's sessions too), and
// prompts sent from the island, which run headless (`claude -p`) in the background.
//
// `claude-sessions` carries the list whenever it changes, and `claude-done` names a session
// that just finished a turn. `claude-reply` carries the answer to a prompt sent from here.
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

/// How many of the most recent sessions are shown.
const SHOWN: usize = 8;
const POLL: Duration = Duration::from_secs(3);
/// Transcripts run to hundreds of MB, so only their ends are read: this much first, then more
/// if that held no messages.
const TAILS: [u64; 3] = [256 << 10, 2 << 20, 8 << 20];
/// How much of the start is read for the folder the session began in.
const HEAD: u64 = 64 << 10;
/// Waiting on a tool longer than this, it's likely waiting on you (a permission prompt).
const TOOL_PATIENCE: Duration = Duration::from_secs(20);
/// Nothing for this long and it isn't working any more (or just done), just idle.
const GONE_QUIET: Duration = Duration::from_secs(30 * 60);
const REPLY_CHARS: usize = 280;

#[derive(Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub title: String,
    pub cwd: String,
    /// working, waiting (likely on you), done, or stopped.
    pub status: &'static str,
    /// Last change, ms since the epoch.
    pub updated: u64,
    /// The start of its latest reply.
    pub reply: Option<String>,
}

#[derive(Clone, Default)]
struct Tail {
    title: Option<String>,
    last_prompt: Option<String>,
    cwd: Option<String>,
    /// The last message: Some(stop reason) for Claude's, None for yours (or a tool's result).
    last: Option<Option<String>>,
    reply: Option<String>,
}

static SESSIONS: Mutex<Vec<Session>> = Mutex::new(Vec::new());

#[tauri::command]
pub fn claude_sessions() -> Vec<Session> {
    SESSIONS.lock().unwrap().clone()
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut cache: HashMap<PathBuf, (SystemTime, Tail)> = HashMap::new();
        loop {
            let sessions = read_sessions(&mut cache);
            let changed = {
                let mut shown = SESSIONS.lock().unwrap();
                let finished: Vec<&Session> = sessions
                    .iter()
                    .filter(|s| s.status == "done")
                    .filter(|s| shown.iter().any(|o| o.id == s.id && matches!(o.status, "working" | "waiting")))
                    .collect();
                for s in finished {
                    let _ = app.emit("claude-done", s);
                }
                let changed = *shown != sessions;
                *shown = sessions;
                changed
            };
            if changed {
                let _ = app.emit("claude-sessions", claude_sessions());
            }
            std::thread::sleep(POLL);
        }
    });
}

fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from)
}

fn read_sessions(cache: &mut HashMap<PathBuf, (SystemTime, Tail)>) -> Vec<Session> {
    let Some(projects) = home().map(|h| h.join(".claude").join("projects")) else { return Vec::new() };
    let mut files: Vec<(PathBuf, SystemTime)> = std::fs::read_dir(projects)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|dir| std::fs::read_dir(dir.path()).ok())
        .flatten()
        .flatten()
        .filter(|f| f.path().extension().is_some_and(|e| e == "jsonl"))
        .filter_map(|f| Some((f.path(), f.metadata().ok()?.modified().ok()?)))
        .collect();
    files.sort_by(|a, b| b.1.cmp(&a.1));
    files.truncate(SHOWN);
    cache.retain(|p, _| files.iter().any(|(f, _)| f == p));

    let now = SystemTime::now();
    files
        .into_iter()
        .map(|(path, modified)| {
            if cache.get(&path).is_none_or(|(m, _)| *m != modified) {
                cache.insert(path.clone(), (modified, read_tail(&path)));
            }
            let tail = &cache[&path].1;
            let age = now.duration_since(modified).unwrap_or_default();
            let status = match &tail.last {
                Some(Some(stop)) if stop == "tool_use" => {
                    if age < TOOL_PATIENCE {
                        "working"
                    } else if age < GONE_QUIET {
                        "waiting"
                    } else {
                        "stopped"
                    }
                }
                Some(Some(_)) if age < GONE_QUIET => "done",
                Some(None) if age < GONE_QUIET => "working",
                _ => "stopped",
            };
            let cwd = tail.cwd.clone().unwrap_or_default();
            let title = tail
                .title
                .clone()
                .or_else(|| tail.last_prompt.clone())
                .or_else(|| Path::new(&cwd).file_name().map(|n| n.to_string_lossy().into_owned()))
                .unwrap_or_else(|| "Untitled".into());
            Session {
                id: path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                title,
                cwd,
                status,
                updated: modified.duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64),
                reply: tail.reply.clone(),
            }
        })
        .collect()
}

fn read_tail(path: &Path) -> Tail {
    let mut tail = Tail::default();
    let Ok(mut file) = File::open(path) else { return tail };
    let size = file.metadata().map_or(0, |m| m.len());
    // The folder it started in (later entries follow every `cd`), which is where it resumes.
    let mut head = vec![0; HEAD as usize];
    let read = file.read(&mut head).unwrap_or(0);
    let started_in = String::from_utf8_lossy(&head[..read])
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find_map(|e| e["cwd"].as_str().map(str::to_string));
    for want in TAILS {
        let start = size.saturating_sub(want);
        let mut bytes = Vec::new();
        if file.seek(SeekFrom::Start(start)).is_err() || file.read_to_end(&mut bytes).is_err() {
            return tail;
        }
        tail = Tail::default();
        let text = String::from_utf8_lossy(&bytes);
        // The first line is likely cut in half, unless the read began at the start.
        for line in text.lines().skip(usize::from(start > 0)) {
            let Ok(entry) = serde_json::from_str::<Value>(line) else { continue };
            read_entry(&entry, &mut tail);
        }
        if tail.last.is_some() || start == 0 {
            break;
        }
    }
    tail.cwd = started_in.or(tail.cwd);
    tail
}

fn read_entry(entry: &Value, tail: &mut Tail) {
    let text = |v: &Value| v.as_str().map(str::to_string);
    if let Some(cwd) = text(&entry["cwd"]) {
        tail.cwd = Some(cwd);
    }
    match entry["type"].as_str() {
        Some("custom-title") => tail.title = text(&entry["customTitle"]).or(tail.title.take()),
        Some("last-prompt") => tail.last_prompt = text(&entry["lastPrompt"]).or(tail.last_prompt.take()),
        // Side chains are subagents' own conversations, not the session's.
        Some("user") if entry["isSidechain"] != true && entry["isMeta"] != true => tail.last = Some(None),
        Some("assistant") if entry["isSidechain"] != true => {
            let message = &entry["message"];
            tail.last = Some(Some(message["stop_reason"].as_str().unwrap_or("").to_string()));
            let said: String = message["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|c| c["type"] == "text")
                .filter_map(|c| c["text"].as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let said = said.split_whitespace().collect::<Vec<_>>().join(" ");
            if !said.is_empty() {
                tail.reply = Some(said.chars().take(REPLY_CHARS).collect());
            }
        }
        _ => {}
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Reply {
    /// The session it ran in (a new one unless it continued one).
    session: Option<String>,
    text: String,
    ok: bool,
}

/// Runs `prompt` headless in `cwd`, continuing `session` if given, and answers with
/// `claude-reply`. Headless, Claude can't ask before it acts, so anything that needs your OK
/// (editing files, running commands) is refused and it says so.
#[tauri::command]
pub fn claude_send(app: AppHandle, prompt: String, cwd: String, session: Option<String>) {
    std::thread::spawn(move || {
        let reply = run(&prompt, &cwd, session.as_deref()).unwrap_or_else(|e| Reply { session: session.clone(), text: e, ok: false });
        let _ = app.emit("claude-reply", reply);
    });
}

fn run(prompt: &str, cwd: &str, session: Option<&str>) -> Result<Reply, String> {
    let installed = home().map(|h| h.join(".local").join("bin").join("claude.exe")).filter(|p| p.exists());
    let mut cmd = Command::new(installed.unwrap_or_else(|| PathBuf::from("claude")));
    cmd.args(["-p", "--output-format", "json"]);
    if let Some(id) = session {
        cmd.args(["--resume", id]);
    }
    // The prompt goes in on stdin, so nothing in it can be read as a flag.
    cmd.current_dir(cwd).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|e| format!("Couldn't start Claude Code: {e}"))?;
    child.stdin.take().ok_or("no stdin")?.write_all(prompt.as_bytes()).map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let json: Value = serde_json::from_slice(&out.stdout).map_err(|_| {
        let err = String::from_utf8_lossy(&out.stderr);
        format!("Claude Code failed: {}", err.trim().lines().last().unwrap_or("no output"))
    })?;
    Ok(Reply {
        session: json["session_id"].as_str().map(str::to_string),
        text: json["result"].as_str().unwrap_or("").trim().to_string(),
        ok: json["is_error"] != true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_title_status_and_reply() {
        let lines = [
            r#"{"type":"custom-title","customTitle":"Fix the build"}"#,
            r#"{"type":"user","cwd":"C:\\code\\app","message":{"role":"user","content":"go"}}"#,
            r#"{"type":"assistant","message":{"stop_reason":"tool_use","content":[{"type":"tool_use"}]}}"#,
            r#"{"type":"assistant","isSidechain":true,"message":{"stop_reason":"end_turn","content":[{"type":"text","text":"sub"}]}}"#,
            r#"{"type":"assistant","message":{"stop_reason":"end_turn","content":[{"type":"text","text":"All   green."}]}}"#,
        ];
        let mut tail = Tail::default();
        for l in lines {
            read_entry(&serde_json::from_str(l).unwrap(), &mut tail);
        }
        assert_eq!(tail.title.as_deref(), Some("Fix the build"));
        assert_eq!(tail.cwd.as_deref(), Some("C:\\code\\app"));
        assert_eq!(tail.last, Some(Some("end_turn".into())));
        assert_eq!(tail.reply.as_deref(), Some("All green."));
    }
}
