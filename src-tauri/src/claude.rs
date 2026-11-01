// Claude Code in the island: your recent sessions and how each is getting on (what it's doing
// now, and its task list), read from the transcripts Claude Code keeps in ~/.claude/projects
// (the desktop app's sessions too). Anything more is for Claude itself, which it opens.
//
// `claude-sessions` carries the list whenever it changes, and `claude-done` names a session
// that just finished a turn.
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

/// How many of the most recent sessions are shown.
const SHOWN: usize = 8;
const POLL: Duration = Duration::from_secs(3);
/// Transcripts run to hundreds of MB, so only their ends are read: this much first, then more
/// if that held no messages, or not the start of the task list. After that, just what's added.
const TAILS: [u64; 3] = [256 << 10, 2 << 20, 8 << 20];
/// How much of the start is read for the folder the session began in.
const HEAD: u64 = 64 << 10;
/// Waiting on a tool longer than this, it's likely waiting on you (a permission prompt).
const TOOL_PATIENCE: Duration = Duration::from_secs(20);
/// Nothing for this long and it isn't working any more (or just done), just idle.
const GONE_QUIET: Duration = Duration::from_secs(30 * 60);
/// The list carries this much of each latest reply.
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
    /// The desktop app's own id for it, if the desktop app runs it.
    pub desktop: Option<String>,
    /// What it did last, in a few words ("Editing app.rs").
    pub step: Option<String>,
    /// The task list it keeps for what you last asked, if it keeps one.
    pub tasks: Vec<Task>,
}

/// A step of a session's task list.
#[derive(Serialize, Clone, PartialEq, Debug)]
pub struct Task {
    pub text: String,
    /// pending, in_progress, or completed.
    pub status: String,
}

#[derive(Clone, Default)]
struct Tail {
    title: Option<String>,
    last_prompt: Option<String>,
    cwd: Option<String>,
    /// The last message: Some(stop reason) for Claude's, None for yours (or a tool's result).
    last: Option<Option<String>>,
    reply: Option<String>,
    step: Option<String>,
    tasks: Vec<Task>,
    /// Saw where the task list begins: your ask, or the list sent whole.
    anchored: bool,
    /// How far it's been read: to the end of the last whole line.
    read_to: u64,
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
                let tail = cache
                    .remove(&path)
                    .map(|(_, tail)| tail)
                    .and_then(|mut tail| read_more(&path, &mut tail).then_some(tail))
                    .unwrap_or_else(|| read_tail(&path));
                cache.insert(path.clone(), (modified, tail));
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
                reply: tail.reply.clone(),
                step: tail.step.clone(),
                tasks: tail.tasks.clone(),
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
    // The folders it was in near the start (later entries follow every `cd`).
    let mut head = vec![0; HEAD as usize];
    let read = file.read(&mut head).unwrap_or(0);
    let early: Vec<String> = String::from_utf8_lossy(&head[..read])
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|e| e["cwd"].as_str().map(str::to_string))
        .collect();
    for want in TAILS {
        let start = size.saturating_sub(want);
        tail = Tail::default();
        // The first line is likely cut in half, unless the read began at the start.
        if !read_lines(&mut file, start, start > 0, &mut tail) {
            return tail;
        }
        if (tail.last.is_some() && tail.anchored) || start == 0 {
            break;
        }
    }
    // It resumes only in the folder its transcript is filed under, which needn't be one it names
    // (the desktop app files it under where it began, and its entries can name folders inside
    // that): take the first of those folders, or a folder they're in, that's filed there.
    let filed = path.parent().and_then(Path::file_name).map(|n| n.to_string_lossy().into_owned());
    let cwd = early
        .iter()
        .chain(tail.cwd.iter())
        .flat_map(|c| Path::new(c).ancestors())
        .map(|p| p.to_string_lossy().trim_end_matches('\\').to_string())
        .find(|c| filed.as_deref() == Some(filed_as(c).as_str()))
        .or(early.first().cloned());
    tail.cwd = cwd.or(tail.cwd);
    tail
}

/// Reads on from where `tail` left off, if the transcript only grew since.
fn read_more(path: &Path, tail: &mut Tail) -> bool {
    let Ok(mut file) = File::open(path) else { return false };
    if tail.read_to == 0 || file.metadata().map_or(0, |m| m.len()) < tail.read_to {
        return false;
    }
    // Where it resumes was settled on the first read.
    let cwd = tail.cwd.clone();
    let read = read_lines(&mut file, tail.read_to, false, tail);
    tail.cwd = cwd;
    read
}

/// Reads the whole lines from `start` on into `tail`, leaving one still being written.
fn read_lines(file: &mut File, start: u64, skip_first: bool, tail: &mut Tail) -> bool {
    let mut bytes = Vec::new();
    if file.seek(SeekFrom::Start(start)).is_err() || file.read_to_end(&mut bytes).is_err() {
        return false;
    }
    let whole = bytes.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    for line in String::from_utf8_lossy(&bytes[..whole]).lines().skip(usize::from(skip_first)) {
        let Ok(entry) = serde_json::from_str::<Value>(line) else { continue };
        read_entry(&entry, tail);
    }
    tail.read_to = start + whole as u64;
    true
}

/// The name of the folder in ~/.claude/projects that Claude Code files `cwd`'s sessions under.
fn filed_as(cwd: &str) -> String {
    cwd.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
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
        Some("user") if entry["isSidechain"] != true && entry["isMeta"] != true => {
            tail.last = Some(None);
            // A new ask starts afresh (a summary after compacting carries on with the old one).
            if asks(&entry["message"]["content"]) && entry["isCompactSummary"] != true {
                tail.step = None;
                tail.tasks.clear();
                tail.anchored = true;
            }
        }
        Some("assistant") if entry["isSidechain"] != true => {
            let message = &entry["message"];
            tail.last = Some(Some(message["stop_reason"].as_str().unwrap_or("").to_string()));
            tail.step = doing(message).or(tail.step.take());
            for call in message["content"].as_array().into_iter().flatten().filter(|c| c["type"] == "tool_use") {
                let name = call["name"].as_str().unwrap_or("");
                tail.anchored |= name == "TodoWrite" || (name.ends_with("update_plan") && call["input"]["items"].is_array());
                plan(name, &call["input"], &mut tail.tasks);
            }
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
                tail.reply = Some(snippet(said));
            }
        }
        _ => {}
    }
}

/// A prompt of yours, not a tool's result or a slash command (which come wrapped in tags).
fn asks(content: &Value) -> bool {
    let typed = |t: &str| !t.trim_start().starts_with('<');
    match content.as_str() {
        Some(text) => typed(text),
        None => content.as_array().is_some_and(|blocks| {
            blocks.iter().all(|b| b["type"] != "tool_result") && blocks.iter().filter_map(|b| b["text"].as_str()).any(typed)
        }),
    }
}

/// Plays a task-list call onto the list: the CLI's TodoWrite sends it whole; the desktop app's
/// update_plan sends it whole or says which steps moved on (1-based), and wrap_up ends it.
fn plan(name: &str, input: &Value, tasks: &mut Vec<Task>) {
    let list = |items: &Value| -> Option<Vec<Task>> {
        let items = items.as_array()?;
        Some(items.iter().map(|i| Task { text: i["content"].as_str().unwrap_or("").into(), status: i["status"].as_str().unwrap_or("pending").into() }).collect())
    };
    let set = |tasks: &mut Vec<Task>, i: Option<usize>, status: &str| {
        if let Some(t) = i.and_then(|i| tasks.get_mut(i)) {
            t.status = status.into();
        }
    };
    let nth = |n: &Value| n.as_u64().and_then(|n| (n as usize).checked_sub(1));
    if name == "TodoWrite" {
        *tasks = list(&input["todos"]).unwrap_or_default();
    } else if name.ends_with("update_plan") {
        if let Some(items) = list(&input["items"]) {
            *tasks = items;
        }
        if input["advance"] == true {
            let at = tasks.iter().position(|t| t.status == "in_progress");
            set(tasks, at, "completed");
            let next = tasks.iter().position(|t| t.status == "pending");
            set(tasks, next, "in_progress");
        }
        for n in input["done"].as_array().into_iter().flatten() {
            set(tasks, nth(n), "completed");
        }
        set(tasks, nth(&input["start"]), "in_progress");
    } else if name.ends_with("wrap_up") {
        tasks.iter_mut().for_each(|t| t.status = "completed".into());
    }
}

fn exe() -> PathBuf {
    let installed = home().map(|h| h.join(".local").join("bin").join("claude.exe")).filter(|p| p.exists());
    installed.unwrap_or_else(|| PathBuf::from("claude"))
}

/// What a step is doing, in a few words.
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
        ("TodoWrite", _) => "Planning".into(),
        (name, _) if name.ends_with("update_plan") => "Planning".into(),
        (name, _) if name.ends_with("wrap_up") => "Wrapping up".into(),
        // A connector's tools come as mcp__server__tool.
        (name, _) => format!("Using {}", name.rsplit("__").next().unwrap_or(name).replace('_', " ")),
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
    fn resumes_in_the_folder_it_is_filed_under() {
        let dir = std::env::temp_dir().join("island-test").join("C--code-app");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.jsonl");
        let lines = [
            r#"{"type":"user","cwd":"C:\\code\\other","message":{"role":"user","content":"go"}}"#,
            r#"{"type":"user","cwd":"C:\\code\\app","message":{"role":"user","content":"go"}}"#,
        ];
        std::fs::write(&path, lines.join("\n") + "\n").unwrap();
        assert_eq!(filed_as("C:\\code\\app"), "C--code-app");
        assert_eq!(read_tail(&path).cwd.as_deref(), Some("C:\\code\\app"));
        // Filed under a folder that holds the ones it names.
        let up = dir.parent().unwrap().join("C--code");
        std::fs::create_dir_all(&up).unwrap();
        std::fs::write(up.join("s.jsonl"), lines.join("\n") + "\n").unwrap();
        assert_eq!(read_tail(&up.join("s.jsonl")).cwd.as_deref(), Some("C:\\code"));
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
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

    #[test]
    fn keeps_the_task_list() {
        let call = |name: &str, input: &str| {
            format!(r#"{{"type":"assistant","message":{{"stop_reason":"tool_use","content":[{{"type":"tool_use","name":"{name}","input":{input}}}]}}}}"#)
        };
        let items = r#"{"items":[{"content":"Look","status":"in_progress"},{"content":"Fix","status":"pending"},{"content":"Test","status":"pending"}]}"#;
        let lines = [
            r#"{"type":"user","message":{"content":"fix it"}}"#.to_string(),
            call("mcp__ccd_turn__update_plan", items),
            r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"ok"}]}}"#.to_string(),
            call("mcp__ccd_turn__update_plan", r#"{"advance":true}"#),
            call("Edit", r#"{"file_path":"C:\\a\\app.rs"}"#),
        ];
        let mut tail = Tail::default();
        for l in &lines {
            read_entry(&serde_json::from_str(l).unwrap(), &mut tail);
        }
        let statuses = |t: &Tail| t.tasks.iter().map(|t| t.status.clone()).collect::<Vec<_>>();
        assert_eq!(statuses(&tail), ["completed", "in_progress", "pending"]);
        assert_eq!(tail.tasks[1].text, "Fix");
        assert_eq!(tail.step.as_deref(), Some("Editing app.rs"));
        read_entry(&serde_json::from_str(&call("mcp__ccd_turn__update_plan", r#"{"done":[2],"start":3}"#)).unwrap(), &mut tail);
        assert_eq!(statuses(&tail), ["completed", "completed", "in_progress"]);
        read_entry(&serde_json::from_str(&call("mcp__ccd_turn__wrap_up", "{}")).unwrap(), &mut tail);
        assert_eq!(statuses(&tail), ["completed"; 3]);
        // Read from a file, it looks back past the end for where the list began, then reads on.
        let path = std::env::temp_dir().join("island-plan-test.jsonl");
        let filler = r#"{"type":"assistant","message":{"stop_reason":"tool_use","content":[{"type":"tool_use","name":"Read"}]}}"#;
        let mut text = lines[..4].join("\n") + "\n";
        text += &(filler.to_string() + "\n").repeat((300 << 10) / filler.len());
        std::fs::write(&path, &text).unwrap();
        let mut read = read_tail(&path);
        assert_eq!(statuses(&read), ["completed", "in_progress", "pending"]);
        let half = call("mcp__ccd_turn__wrap_up", "{}");
        std::fs::write(&path, text.clone() + &half[..20]).unwrap();
        assert!(read_more(&path, &mut read) && statuses(&read)[2] == "pending");
        std::fs::write(&path, text + &half + "\n").unwrap();
        assert!(read_more(&path, &mut read) && statuses(&read) == ["completed"; 3]);
        let _ = std::fs::remove_file(&path);
        // A new ask clears it.
        read_entry(&serde_json::from_str(r#"{"type":"user","message":{"content":[{"type":"text","text":"next"}]}}"#).unwrap(), &mut tail);
        assert!(tail.tasks.is_empty() && tail.step.is_none());
    }
}
