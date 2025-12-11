use std::thread;

use anyhow::Result;

use crate::app::{App, BackendEvent, Message, MessageFrom, EditContext, Branch};

use reqwest::blocking::{Client, Response};
use serde::Deserialize;
use std::error::Error;
use std::io::{BufRead, BufReader, Write};

use std::fs;
use std::fs::File;
use std::path::Path;

/// Response format for Ollama's /api/generate endpoint.
#[derive(Deserialize, Debug)]
struct OllamaResponse {
    response: Option<String>,
    done: Option<bool>,
}

/// Streams Ollama responses line by line and returns the full combined text.
fn stream_response_lines<R: std::io::Read>(
    reader: R,
    mut on_chunk: impl FnMut(String),
) -> Result<String, Box<dyn std::error::Error>> {
    let reader = BufReader::new(reader);
    let mut buffer = String::new();

    for line_result in reader.lines() {
        let line = line_result?;
        if let Ok(parsed) = serde_json::from_str::<OllamaResponse>(&line) {
            if let Some(text) = parsed.response {
                on_chunk(text.clone());
                buffer.push_str(&text);
            }
            if parsed.done.unwrap_or(false) {
                break;
            }
        }
    }
    Ok(buffer)
}

/// Send a user message on the active branch and start background streaming.
pub fn send_user_message_with_streaming(app: &mut App, text: String) -> Result<()> {
    let prompt = text.clone();
    let session_idx = app.active_idx;
    let branch_idx = app.sessions[session_idx].active_branch;

    // 1) Append user message to the active branch
    {
        let session = &mut app.sessions[session_idx];
        let branch = &mut session.branches[branch_idx];
        branch.messages.push(Message {
            from: MessageFrom::User,
            content: text,
        });
    }

    let history = app.history_string();
    let full_prompt = if history.is_empty() {
        format!("User: {prompt}\nAssistant:")
    } else {
        format!("{history}\nUser: {prompt}\nAssistant:")
    };

    // 2) Create an empty assistant message for streaming output
    app.start_streaming_assistant(session_idx, branch_idx);

    // 3) Clear the input box
    app.input.clear();

    // 4) Spawn a background worker to call Ollama and stream chunks
    if let Some(tx) = app.backend_tx.clone() {
        thread::spawn(move || {
            let client = reqwest::blocking::Client::new();

            let response = client
                .post("http://localhost:11434/api/generate")
                .json(&serde_json::json!({
                    "model": "qwen3:1.7b",
                    "prompt": full_prompt,
                    "stream": true
                }))
                .send();

            if let Ok(resp) = response {
                let _ = stream_response_lines(resp, |chunk| {
                    let _ = tx.send(BackendEvent::AssistantChunk {
                        session_idx,
                        branch_idx,
                        chunk,
                    });
                });

                // send final "done"
                let _ = tx.send(BackendEvent::AssistantDone {
                    session_idx,
                    branch_idx,
                });
            }
        });
    }

    Ok(())
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

    // Spawn background worker (same logic as send_user_message_with_streaming)
    if let Some(tx) = app.backend_tx.clone() {
        thread::spawn(move || {
            let client = reqwest::blocking::Client::new();

            let response = client
                .post("http://localhost:11434/api/generate")
                .json(&serde_json::json!({
                    "model": "qwen3:1.7b",
                    "prompt": prompt,
                    "stream": true
                }))
                .send();

            if let Ok(resp) = response {
                let _ = stream_response_lines(resp, |chunk| {
                    let _ = tx.send(BackendEvent::AssistantChunk {
                        session_idx,
                        branch_idx,
                        chunk,
                    });
                });

                // send final "done"
                let _ = tx.send(BackendEvent::AssistantDone {
                    session_idx,
                    branch_idx,
                });
            }
        });
    }

    Ok(())
}

