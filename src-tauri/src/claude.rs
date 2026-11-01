// Claude Code in the island: your recent sessions and what each is up to, read from the
// transcripts Claude Code keeps in ~/.claude/projects (the desktop app's sessions too), and
// prompts sent from the island, which run headless (`claude -p`) in the background.
//
// `claude-sessions` carries the list whenever it changes, and `claude-done` names a session
// that just finished a turn. `claude-progress` says what a prompt sent from here is doing, and
// `claude-reply` carries its answer.
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
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
/// The list carries this much of each latest reply; picking a session reads up to FULL_CHARS.
const REPLY_CHARS: usize = 280;
const FULL_CHARS: usize = 20_000;

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
    /// The desktop app's own id for it, if the desktop app runs it.
    pub desktop: Option<String>,
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

type DesktopCache = HashMap<PathBuf, (SystemTime, Option<(String, String)>)>;

static SESSIONS: Mutex<Vec<Session>> = Mutex::new(Vec::new());

#[tauri::command]
pub fn claude_sessions() -> Vec<Session> {
    SESSIONS.lock().unwrap().clone()
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut cache: HashMap<PathBuf, (SystemTime, Tail)> = HashMap::new();
        let mut desktop_cache = DesktopCache::new();
        loop {
            let desktop = desktop_ids(&mut desktop_cache);
            let sessions = read_sessions(&mut cache, &desktop);
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

/// The desktop app's own ids for the sessions it runs (its `claude://` links take those), by
/// Claude Code session id. It keeps a small file per session.
fn desktop_ids(cache: &mut DesktopCache) -> HashMap<String, String> {
    let roots = [
        std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Claude-Data")),
        std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("Claude")),
    ];
    // Store installs keep it a level down, in a folder per package.
    let data: Vec<PathBuf> = roots.into_iter().flatten().flat_map(|r| [vec![r.clone()], dirs(&r)].concat()).collect();
    let files: Vec<(PathBuf, SystemTime)> = data
        .iter()
        .flat_map(|d| dirs(&d.join("claude-code-sessions")))
        .flat_map(|account| dirs(&account))
        .flat_map(|org| std::fs::read_dir(org).into_iter().flatten().flatten())
        .filter(|f| f.file_name().to_string_lossy().starts_with("local_"))
        .filter_map(|f| Some((f.path(), f.metadata().ok()?.modified().ok()?)))
        .collect();
    cache.retain(|p, _| files.iter().any(|(f, _)| f == p));
    for (path, modified) in files {
        if cache.get(&path).is_none_or(|(m, _)| *m != modified) {
            let ids = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).and_then(|j| {
                // Archived ones won't open.
                (j["isArchived"] != true).then_some(())?;
                Some((j["cliSessionId"].as_str()?.to_string(), j["sessionId"].as_str()?.to_string()))
            });
            cache.insert(path, (modified, ids));
        }
    }
    cache.values().filter_map(|(_, ids)| ids.clone()).collect()
}

fn dirs(path: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(path).into_iter().flatten().flatten().map(|d| d.path()).filter(|p| p.is_dir()).collect()
}

fn read_sessions(cache: &mut HashMap<PathBuf, (SystemTime, Tail)>, desktop: &HashMap<String, String>) -> Vec<Session> {
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
    files.sort_by_key(|f| std::cmp::Reverse(f.1));
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
            let id = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            Session {
                desktop: desktop.get(&id).cloned(),
                id,
                title,
                cwd,
                status,
                updated: modified.duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64),
                reply: tail.reply.as_deref().map(snippet),
            }
        })
        .collect()
}

fn snippet(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(REPLY_CHARS).collect()
}

/// Session ids are UUIDs; anything else isn't looked up (or put on a command line).
fn valid(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// The whole of a session's latest reply, for when it's picked.
#[tauri::command]
pub fn claude_reply(id: String) -> Option<String> {
    if !valid(&id) {
        return None;
    }
    let projects = home()?.join(".claude").join("projects");
    let path = std::fs::read_dir(projects).ok()?.flatten().map(|d| d.path().join(format!("{id}.jsonl"))).find(|p| p.exists())?;
    read_tail(&path).reply
}

/// Opens the session to carry on there with everything Claude Code can do: in the desktop app
/// when given its id there, else in a terminal.
#[tauri::command]
pub fn claude_open(id: String, cwd: String, desktop: Option<String>) -> Result<(), String> {
    if let Some(local) = desktop {
        if !local.strip_prefix("local_").is_some_and(valid) {
            return Err("Not a session".into());
        }
        let link = format!("claude://code/continue?session={local}");
        return tauri_plugin_opener::open_url(link, None::<&str>).map_err(|e| format!("Couldn't open the desktop app: {e}"));
    }
    if !valid(&id) {
        return Err("Not a session".into());
    }
    let claude = exe();
    let terminal = Command::new("wt.exe").arg("-d").arg(&cwd).arg(&claude).args(["--resume", &id]).spawn();
    if terminal.is_err() {
        // No Windows Terminal: a plain console window.
        Command::new("cmd")
            .current_dir(&cwd)
            .args(["/c", "start", "Claude Code"])
            .arg(&claude)
            .args(["--resume", &id])
            .spawn()
            .map_err(|e| format!("Couldn't open a terminal: {e}"))?;
    }
    Ok(())
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
                .join("\n\n");
            let said = said.trim();
            if !said.is_empty() {
                tail.reply = Some(said.chars().take(FULL_CHARS).collect());
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
        let reply = run(&app, &prompt, &cwd, session.as_deref()).unwrap_or_else(|e| Reply { session: session.clone(), text: e, ok: false });
        let _ = app.emit("claude-reply", reply);
    });
}

fn exe() -> PathBuf {
    let installed = home().map(|h| h.join(".local").join("bin").join("claude.exe")).filter(|p| p.exists());
    installed.unwrap_or_else(|| PathBuf::from("claude"))
}

fn run(app: &AppHandle, prompt: &str, cwd: &str, session: Option<&str>) -> Result<Reply, String> {
    let mut cmd = Command::new(exe());
    // A stream of events, so the island can say what it's doing as it goes.
    cmd.args(["-p", "--output-format", "stream-json", "--verbose"]);
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
    // Read on the side, so a chatty stderr can't fill its pipe and stall the run.
    let mut stderr = child.stderr.take().ok_or("no stderr")?;
    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });
    let mut result = None;
    for line in BufReader::new(child.stdout.take().ok_or("no stdout")?).lines().map_while(Result::ok) {
        let Ok(event) = serde_json::from_str::<Value>(&line) else { continue };
        match event["type"].as_str() {
            Some("assistant") => {
                if let Some(doing) = doing(&event["message"]) {
                    let _ = app.emit("claude-progress", doing);
                }
            }
            Some("result") => result = Some(event),
            _ => {}
        }
    }
    let _ = child.wait();
    let json = result.ok_or_else(|| {
        let err = errors.join().unwrap_or_default();
        format!("Claude Code failed: {}", err.trim().lines().last().unwrap_or("no output"))
    })?;
    let text = json["result"].as_str().unwrap_or("").trim();
    let ok = json["is_error"] != true;
    // The command line signs in apart from the desktop app.
    let text = if !ok && text.starts_with("Not logged in") {
        "Claude Code isn't signed in here. Open a terminal, run claude, then type /login.".to_string()
    } else {
        text.to_string()
    };
    Ok(Reply { session: json["session_id"].as_str().map(str::to_string), text, ok })
}

/// What a step of a run is doing, in a few words.
fn doing(message: &Value) -> Option<String> {
    let last = message["content"].as_array()?.last()?;
    if last["type"] == "text" {
        return Some("Writing the answer".into());
    }
    if last["type"] != "tool_use" {
        return None;
    }
    let input = &last["input"];
    let file = input["file_path"]
        .as_str()
        .or(input["path"].as_str())
        .and_then(|p| Path::new(p).file_name())
        .map(|n| n.to_string_lossy().into_owned());
    Some(match (last["name"].as_str().unwrap_or("a tool"), file) {
        ("Read", Some(f)) => format!("Reading {f}"),
        ("Edit" | "Write" | "MultiEdit" | "NotebookEdit", Some(f)) => format!("Editing {f}"),
        ("Grep" | "Glob", _) => "Searching".into(),
        ("Bash" | "PowerShell", _) => "Running a command".into(),
        ("WebSearch" | "WebFetch", _) => "Looking online".into(),
        ("Agent" | "Task", _) => "Sending a helper".into(),
        (name, _) => format!("Using {name}"),
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
        assert_eq!(tail.reply.as_deref().map(snippet).as_deref(), Some("All green."));
    }

    #[test]
    fn says_what_a_step_is_doing() {
        let step = |c: &str| doing(&serde_json::from_str(&format!(r#"{{"content":[{c}]}}"#)).unwrap());
        assert_eq!(step(r#"{"type":"tool_use","name":"Read","input":{"file_path":"C:\\a\\b.rs"}}"#).as_deref(), Some("Reading b.rs"));
        assert_eq!(step(r#"{"type":"tool_use","name":"Grep","input":{"pattern":"x"}}"#).as_deref(), Some("Searching"));
        assert_eq!(step(r#"{"type":"text","text":"hi"}"#).as_deref(), Some("Writing the answer"));
        assert_eq!(step(r#"{"type":"thinking"}"#), None);
        assert!(!valid("../x") && valid("8be4-7f0a"));
    }
}
