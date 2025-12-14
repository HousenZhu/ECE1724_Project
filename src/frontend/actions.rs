use std::thread;

use anyhow::Result;
use std::sync::mpsc::Sender;

use crate::app::{App, BackendEvent, Message, MessageFrom, EditContext, Branch};

use reqwest::blocking::{Client};
use std::error::Error;

use std::fs;

use regex::Regex;
use serde_json::{json, Value};
use crate::frontend::api_key::DASHSCOPE_API_KEY;

// print help information
pub fn show_help_message(app: &mut App) -> Result<()> {
    let session_idx = app.active_idx;
    let branch_idx = app.sessions[session_idx].active_branch;

    // 1) Create an empty assistant message for streaming output
    app.start_streaming_assistant(session_idx, branch_idx);

    // 3) Clear the input box
    app.input.clear();
    app.input_scroll = 0; 

    if let Some(tx_main) = app.backend_tx.clone() {
        let tx_thread = tx_main.clone(); // clone for thread

        thread::spawn(move || {
            let tx_for_loop = tx_thread.clone();
            let tx_for_done = tx_thread.clone();
           
            let _ = stream_help_message(session_idx, branch_idx, tx_for_loop);

            // send final done event
            let _ = tx_for_done.send(BackendEvent::AssistantDone {
                session_idx,
                branch_idx,
            });
            
        });
    }

    Ok(())
}

pub fn stream_help_message(
    session_idx: usize,
    branch_idx: usize,
    tx: Sender<BackendEvent>,
) -> Result<(), Box<dyn std::error::Error>> {

    let help_text = r#"
📖 MyCLI Help

NORMAL MODE
  q          Quit
  n          New session
  j / k      Next / previous session
  ↑ / ↓      Move session selection
  [ / ]      Previous / next branch
  TAB        Toggle new-session button
  s          Toggle sidebar
  e          Edit last user message
  i          Enter insert mode

INSERT MODE
  Enter      Send message
  Esc        Back to normal mode

TIPS
  • Editing a message forks a new branch

"#;

    // stream like LLM output
    stream_string_into_ui(help_text, session_idx, branch_idx, &tx)?;

    Ok(())
}



pub fn call_chat_api(
    client: &Client,
    model: &str,
    messages: &[Value],
) -> Result<String, Box<dyn Error>> {

    let api_key = DASHSCOPE_API_KEY;

    let url = "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions";

    let resp = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": model,
            "messages": messages,
        }))
        .send()?;

    let status = resp.status();
    let body: Value = resp.json()?;

    if !status.is_success() {
        let msg = body
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error from API");
        return Err(format!("DashScope API error ({status}): {msg}").into());
    }

    Ok(body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string())
}


/// Send a user message on the active branch and start background streaming.
pub fn send_user_message_with_streaming(app: &mut App, text: String) -> Result<()> {
    let prompt = text.clone();
    let session_idx = app.active_idx;
    let branch_idx = app.sessions[session_idx].active_branch;

    // 1) Append user message
    {
        let session = &mut app.sessions[session_idx];
        let branch = &mut session.branches[branch_idx];
        branch.messages.push(Message {
            from: MessageFrom::User,
            content: text,
        });
    }

    // 2) Create empty assistant message for streaming output
    app.start_streaming_assistant(session_idx, branch_idx);

    // 3) Clear UI input
    app.input.clear();
    app.input_scroll = 0;

    // 4) precompute history BEFORE thread
    let initial_history = app.history_string();

    // 5) Clone channel
    if let Some(tx_main) = app.backend_tx.clone() {
        let tx_thread = tx_main.clone(); // clone for thread

        thread::spawn(move || {
            let tx_for_loop = tx_thread.clone();
            let tx_for_done = tx_thread.clone();

            if let Err(e) = run_mcp_loop(prompt, initial_history, session_idx, branch_idx, tx_for_loop) {
                eprintln!("MCP error: {e}");
            }

            // send final done event
            let _ = tx_for_done.send(BackendEvent::AssistantDone {
                session_idx,
                branch_idx,
            });
        });
    }

    Ok(())
}

fn run_mcp_loop(
    user_prompt: String,
    mut history: String,
    session_idx: usize,
    branch_idx: usize,
    tx: Sender<BackendEvent>,
) -> Result<(), Box<dyn Error>> {
    let client = Client::new();

    let system_mcp_prompt = format!(
        "You are an AI assistant with access to MCP tools.\n\
        Available tools:\n\
        - filesystem.read  - read file content. Example: <use_tool name=\"filesystem.read\" params={{\"path\": \"src/main.rs\"}} />\n\
        - filesystem.write - write text into a file. Example: <use_tool name=\"filesystem.write\" params={{\"path\": \"output.txt\", \"content\": \"Hello\"}} />\n\
        - shell.run - run shell commands. Example: <use_tool name=\"shell.run\" params={{\"content\": \"mkdir Playground\"}} />\n\
        On macOS/Linux, shell commands are executed via `sh -c \"command\"`.\n\
        On Windows, they run via `cmd /C \"command\"`.\n\
        When using a tool, use EXACTLY this XML-style syntax.\n\
        You can add some explaining information after a tool call, but take care of format for readability.\n\
        You can use **only one <use_tool> command per message.** If you need to use multiple tools, call them one by one — wait for the tool's result before issuing the next <use_tool>.\n\
        You can use **only one <use_tool> command per message.** \n\
        You can use **only one <use_tool> command per message.** "

    );


    loop {
        // --- Build messages for DashScope ---
        let messages = vec![
            serde_json::json!({
                "role": "system",
                "content": system_mcp_prompt,
            }),
            serde_json::json!({
                "role": "user",
                "content": format!(
                    "User initial prompt:\n{}\n\nConversation so far:\n{}\n\n\
                    Continue reasoning or issue next tool call if needed.",
                    user_prompt, history
                ),
            }),
        ];

        // --- Call DashScope (non-stream) ---
        let answer = call_chat_api(&client, "qwen-plus", &messages)?;

        // --- stream chunks to UI ---
        stream_string_into_ui(&answer, session_idx, branch_idx, &tx)?;

        // --- append answer into history ---
        history.push_str("\nAssistant: ");
        history.push_str(&answer);


        // --- detect tool call ---
        if let Some(tool_call) = parse_tool_use(&answer) {
            let tool_result = execute_mcp(&tool_call)?;

            // stream tool result too
            stream_string_into_ui(
                &format!("\n[Tool: {}]\nresult: {}\n", tool_call.name, tool_result),
                session_idx,
                branch_idx,
                &tx,
            )?;

            // append to history for next round
            history.push_str("\nTool result: ");
            history.push_str(&tool_result);
        } else {
            // no more tools
            break;
        }

        if answer.to_lowercase().contains("done.") {
            break;
        }
    }

    Ok(())
}


fn stream_string_into_ui(
    s: &str,
    session_idx: usize,
    branch_idx: usize,
    tx: &Sender<BackendEvent>,
) -> Result<(), Box<dyn Error>> {
    let max_chunk = 12;
    let delay = std::time::Duration::from_millis(10);

    let mut buf = String::new();

    for c in s.chars() {
        buf.push(c);

        if buf.len() >= max_chunk || c == '\n' {
            tx.send(BackendEvent::AssistantChunk {
                session_idx,
                branch_idx,
                chunk: buf.clone(),
            })?;
            buf.clear();

            // give UI time to animate
            std::thread::sleep(delay);
        }
    }

    if !buf.is_empty() {
        tx.send(BackendEvent::AssistantChunk {
            session_idx,
            branch_idx,
            chunk: buf,
        })?;
    }

    Ok(())
}


/// Struct for parsed tool info
#[derive(Debug)]
struct ToolCall {
    name: String,
    path: Option<String>,
    content: Option<String>,
}

/// Parse MCP-style tool command from model output
fn parse_tool_use(output: &str) -> Option<ToolCall> {
    // First try to capture the whole params JSON object (dot matches newlines with (?s))
    let re_full = Regex::new(r#"(?s)<use_tool\s+name="([^"]+)"\s+params=(\{.*?\})\s*/?>"#).ok()?;
    if let Some(caps) = re_full.captures(output) {
        let name = caps.get(1)?.as_str().to_string();
        let params_str = caps.get(2)?.as_str();

        // Try parsing params as JSON
        if let Ok(json_val) = serde_json::from_str::<Value>(params_str) {
            let path = json_val.get("path").and_then(|v| v.as_str()).map(|s| s.to_string());
            let content = json_val.get("content").and_then(|v| v.as_str()).map(|s| s.to_string());
            return Some(ToolCall { name, path, content });
        }
        // If parsing failed, continue to fallback regexes below
    }

    // Fallback: simple read command
    let re_read = Regex::new(
        r#"<use_tool\s+name="filesystem\.read"\s+params=\{\s*"path":\s*"([^"]+)"\s*\}\s*/?>"#
    ).ok()?;

    if let Some(caps) = re_read.captures(output) {
        return Some(ToolCall {
            name: "filesystem.read".into(),
            path: Some(caps[1].to_string()),
            content: None,
        });
    }

    // Fallback: simple write command (content without escaped quotes might be captured)
    let re_write = Regex::new(
        r#"<use_tool\s+name="filesystem\.write"\s+params=\{\s*"path":\s*"([^"]+)"\s*,\s*"content":\s*"([^"]*)"\s*\}\s*/?>"#
    ).ok()?;

    if let Some(caps) = re_write.captures(output) {
        return Some(ToolCall {
            name: "filesystem.write".into(),
            path: Some(caps[1].to_string()),
            content: Some(caps[2].to_string()),
        });
    }

    // Fallback: shell.run (content = command)
    let re_shell = Regex::new(
        r#"<use_tool\s+name="shell\.run"\s+params=\{\s*"content":\s*"([^"]+)"\s*\}\s*/?>"#
    ).ok()?;

    if let Some(caps) = re_shell.captures(output) {
        return Some(ToolCall {
            name: "shell.run".into(),
            path: None,
            content: Some(caps[1].to_string()),
        });
    }

    None
}

/// Simulate MCP tools (filesystem.read, filesystem.write, shell.run)
fn execute_mcp(tool: &ToolCall) -> Result<String, Box<dyn Error>> {
    match tool.name.as_str() {
        "filesystem.read" => {
            let path = tool.path.as_ref().ok_or("Missing path for filesystem.read")?;
            let content = fs::read_to_string(path)?;
            // println!("📂 Read file '{}': {} bytes", path, content.len());
            Ok(format!("Read file '{}' ({} bytes). Content:\n{}", path, content.len(), content))
        }

        "filesystem.write" => {
            let path = tool.path.as_ref().ok_or("Missing path for filesystem.write")?;
            let data_raw = tool.content.as_ref().ok_or("Missing content for filesystem.write")?;
            let data = normalize_escaped_content(data_raw);

            fs::write(path, &data)?;
            // println!("💾 Wrote {} bytes to '{}'", data.len(), path);
            Ok(format!("Wrote {} bytes to '{}'.", data.len(), path))
        }

        "shell.run" => {
            let command_raw = tool
                .content
                .as_ref()
                .ok_or("Missing 'content' for shell.run (expected shell command)")?;
            // println!("🖥️ Running shell command: `{}`", command_raw);

            #[cfg(target_os = "windows")]
            let output = std::process::Command::new("cmd")
                .args(&["/C", command_raw])
                .output()?;

            #[cfg(not(target_os = "windows"))]
            let output = std::process::Command::new("sh")
                .args(&["-c", command_raw])
                .output()?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            // println!("📤 Command output:\n{}", stdout);
            // if !stderr.is_empty() {
            //     println!("⚠️ Command error output:\n{}", stderr);
            // }

            Ok(format!(
                "Command `{}` executed.\nSTDOUT:\n{}\nSTDERR:\n{}",
                command_raw, stdout, stderr
            ))
        }

        _ => Err(format!("Unknown MCP tool: {}", tool.name).into()),
    }
}

/// Loosely decode escaped sequences and handle real newlines safely
fn normalize_escaped_content(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('n') => { out.push('\n'); chars.next(); }
                Some('t') => { out.push('\t'); chars.next(); }
                Some('r') => { out.push('\r'); chars.next(); }
                Some('\\') => { out.push('\\'); chars.next(); }
                Some('"') => { out.push('"'); chars.next(); }
                Some('\'') => { out.push('\''); chars.next(); }
                _ => out.push(c),
            }
        } else {
            out.push(c);
        }
    }

    out
}



/// Create a new branch starting from the edit point,
/// then send the edited user message on that new branch.
pub fn fork_and_send_from_edit(app: &mut App, ctx: EditContext, text: String) -> Result<()> {
    let EditContext {
        session_idx,
        branch_idx,
        message_idx,
    } = ctx;

    // 1) Take a snapshot of the old branch so it is preserved.
    let session = &mut app.sessions[session_idx];
    let old_branch = &session.branches[branch_idx];

    // Clone all messages in the old branch.
    let mut new_messages = old_branch.messages.clone();

    // 2) Overwrite the edited user message in the cloned branch.
    if let Some(msg) = new_messages.get_mut(message_idx) {
        msg.content = text.clone();
    }

    // 3) Drop everything after the edited message (old assistant reply, etc.).
    new_messages.truncate(message_idx + 1);

    // 4) Create a new branch with this updated message list.
    let new_branch_idx = session.branches.len();
    session.branches.push(Branch {
        id: new_branch_idx,
        name: format!("branch-{new_branch_idx}"),
        messages: new_messages,
    });

    // 5) Switch to the new branch so the UI shows the edited version.
    session.active_branch = new_branch_idx;

    // 6) Start streaming a fresh assistant reply on this new branch.
    start_streaming_on_branch(app, session_idx, new_branch_idx, text)?;

    Ok(())
}

/// Start streaming an assistant reply on a specific session/branch.
fn start_streaming_on_branch(
    app: &mut App,
    session_idx: usize,
    branch_idx: usize,
    prompt: String,
) -> Result<()> {
    // Create empty assistant message in this branch
    app.start_streaming_assistant(session_idx, branch_idx);

    // 4) precompute history BEFORE thread
    let initial_history = app.history_string();

    // 5) Clone channel
    if let Some(tx_main) = app.backend_tx.clone() {
        let tx_thread = tx_main.clone(); // clone for thread

        thread::spawn(move || {
            let tx_for_loop = tx_thread.clone();
            let tx_for_done = tx_thread.clone();

            if let Err(e) = run_mcp_loop(prompt, initial_history, session_idx, branch_idx, tx_for_loop) {
                eprintln!("MCP error: {e}");
            }

            // send final done event
            let _ = tx_for_done.send(BackendEvent::AssistantDone {
                session_idx,
                branch_idx,
            });
        });
    }

    Ok(())
}
