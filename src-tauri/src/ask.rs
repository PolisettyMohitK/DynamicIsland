//! When a spoken request isn't one of the built-in commands, Choco asks the AI service in use
//! (ai.rs: DeepSeek V4.1 Flash through OpenCode Go, OpenAI or Gemini). It answers in a sentence
//! and may call the same island actions (timer, tasks, water, music, volume); the frontend
//! runs those.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::ai::{Job, Provider};

const SYSTEM: &str = "You are Choco, a calm, warm little assistant that lives in the notch at the top of \
the user's Windows screen. The user's words come from speech-to-text, so allow for misheard words. \
When they ask for something the tools can do, call the tools. When they ask about their tasks, water, \
timer or music, call show so they can see it. When it needs their screen (opening or using an app or website, finding something on the computer), call use_computer. Then reply like a thoughtful friend: plain and direct, \
no filler, no \"As an AI\". Most replies are one or two sentences. When the question needs more \
(steps, a list, an explanation), use up to about 120 words in short lines; lines starting with \"- \" \
become bullets. No other formatting: no headings, bold, tables, code or emoji.";

#[derive(Serialize)]
pub struct Answer {
    reply: String,
    actions: Vec<Action>,
}

/// An earlier exchange, so follow-up questions make sense.
#[derive(Deserialize)]
pub struct Turn {
    role: String,
    content: String,
}

#[derive(Serialize)]
pub struct Action {
    name: String,
    input: Value,
}

/// A function tool in OpenAI's format. Every property is required; a tool without any has no
/// parameters at all (Gemini turns away an empty object schema).
pub(crate) fn tool(name: &str, description: &str, properties: Value) -> Value {
    let required: Vec<&String> = properties.as_object().map(|o| o.keys().collect()).unwrap_or_default();
    let mut function = json!({ "name": name, "description": description });
    if !required.is_empty() {
        function["parameters"] = json!({ "type": "object", "properties": properties, "required": required });
    }
    json!({ "type": "function", "function": function })
}

fn tools() -> Value {
    json!([
        tool(
            "focus_timer",
            "Control the focus (pomodoro) timer: start a 25 minute focus session, start a 5 minute break, pause, or reset.",
            json!({ "action": { "type": "string", "enum": ["start_focus", "start_break", "pause", "reset"] } }),
        ),
        tool(
            "add_task",
            "Add a task to today's to-do list. due is a 24-hour HH:MM time for a reminder, or an empty string for none.",
            json!({ "text": { "type": "string" }, "due": { "type": "string" } }),
        ),
        tool("drink_water", "Log one cup of water toward today's goal.", json!({})),
        tool(
            "media",
            "Control whatever music or media is playing.",
            json!({ "action": { "type": "string", "enum": ["play", "pause", "next", "previous"] } }),
        ),
        tool(
            "set_volume",
            "Set the system volume, 0 to 100.",
            json!({ "level": { "type": "integer" } }),
        ),
        tool("set_mute", "Mute or unmute the system sound.", json!({ "muted": { "type": "boolean" } })),
        tool(
            "show",
            "Show a small live panel under the reply: today's tasks, today's water, the focus timer, or what's playing.",
            json!({ "panel": { "type": "string", "enum": ["tasks", "water", "timer", "music"] } }),
        ),
        tool(
            "use_computer",
            "Work the user's Windows desktop for them with the mouse and keyboard: open and use apps and websites, search, click, type. Only for what the other tools can't do. task is the whole request, with anything it needs from the conversation.",
            json!({ "task": { "type": "string" } }),
        ),
        tool("remember", crate::memory::REMEMBER, json!({ "fact": { "type": "string" } })),
        tool("forget", crate::memory::FORGET, json!({ "fact": { "type": "string" } })),
    ])
}

/// `context` is a line about the user's current state (time, what's playing, the timer).
/// `history` is the conversation so far; `session` stays the same across a conversation.
#[tauri::command]
pub async fn choco_ask(
    app: AppHandle,
    text: String,
    context: String,
    history: Vec<Turn>,
    session: String,
) -> Result<Answer, String> {
    let (provider, key) = crate::ai::chosen(&app).ok_or("no-key")?;
    let memory = crate::memory::prompt(&app);
    let mut messages = vec![json!({ "role": "system", "content": format!("{SYSTEM}\n\n{memory}\n\n{context}") })];
    for t in history {
        messages.push(json!({ "role": t.role, "content": t.content }));
    }
    messages.push(json!({ "role": "user", "content": text }));
    let mut body = json!({
        "model": provider.model(&app, Job::Chat),
        "messages": messages,
        "tools": tools(),
        "tool_choice": "auto",
    });
    let mut req = reqwest::Client::new()
        .post(provider.chat_url())
        .bearer_auth(key)
        .header("user-agent", concat!("dynamic-island-choco/", env!("CARGO_PKG_VERSION")));
    // OpenAI's newer models refuse max_tokens; the prompt keeps replies short anyway.
    if provider == Provider::Opencode {
        body["max_tokens"] = json!(1024);
        // Go routes and caches by conversation.
        req = req.header("x-opencode-session", session);
    }

    let res = req.json(&body).send().await.map_err(|e| format!("network: {e}"))?;
    let status = res.status();
    let msg: Value = res.json().await.map_err(|e| format!("bad response: {e}"))?;
    if !status.is_success() {
        return Err(format!("{}: {}", status.as_u16(), crate::ai::error_message(&msg)));
    }

    let message = &msg["choices"][0]["message"];
    let actions: Vec<Action> = message["tool_calls"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|call| Action {
            name: call["function"]["name"].as_str().unwrap_or("").to_string(),
            // Arguments arrive as a JSON string.
            input: call["function"]["arguments"]
                .as_str()
                .and_then(|a| serde_json::from_str(a).ok())
                .unwrap_or_else(|| json!({})),
        })
        .collect();
    // Memory is kept here; the rest are island actions for the frontend.
    let actions = actions
        .into_iter()
        .filter(|a| match a.name.as_str() {
            "remember" => {
                crate::memory::remember(&app, a.input["fact"].as_str().unwrap_or(""));
                false
            }
            "forget" => {
                crate::memory::forget(&app, a.input["fact"].as_str().unwrap_or(""));
                false
            }
            _ => true,
        })
        .collect();
    let reply = message["content"].as_str().unwrap_or("").trim().to_string();
    Ok(Answer { reply, actions })
}
