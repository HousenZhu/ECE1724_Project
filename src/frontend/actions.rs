use std::thread;

use anyhow::Result;

use crate::app::{App, BackendEvent, Message, MessageFrom, EditContext, Branch};

use serde::Deserialize;

/// Response format for Ollama's /api/generate endpoint.
#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

/// Call Ollama's local LLM through the HTTP API.
fn call_ollama(prompt: String) -> anyhow::Result<String> {
    let client = reqwest::blocking::Client::new();

    let body = serde_json::json!({
        "model": "gemma3",
        "prompt": prompt,
        "stream": false
    });

    let resp = client
        .post("http://localhost:11434/api/generate")
        .json(&body)
        .send()?
        .json::<OllamaResponse>()?;

    Ok(resp.response)
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

    // 2) Create an empty assistant message for streaming output
    app.start_streaming_assistant(session_idx, branch_idx);

    // 3) Clear the input box
    app.input.clear();

    // 4) Spawn a background worker to call Ollama and stream chunks
    if let Some(tx) = app.backend_tx.clone() {
        thread::spawn(move || {
            if let Ok(reply) = call_ollama(prompt) {
                let chars: Vec<char> = reply.chars().collect();
                let mut buf = String::new();

                for c in chars {
                    buf.push(c);
                    if buf.len() >= 5 {
                        let _ = tx.send(BackendEvent::AssistantChunk {
                            session_idx,
                            branch_idx,
                            chunk: buf.clone(),
                        });
                        buf.clear();
                        std::thread::sleep(std::time::Duration::from_millis(40));
                    }
                }

                if !buf.is_empty() {
                    let _ = tx.send(BackendEvent::AssistantChunk {
                        session_idx,
                        branch_idx,
                        chunk: buf,
                    });
                }

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
fn fork_and_send_from_edit(app: &mut App, ctx: EditContext, text: String) -> Result<()> {
    let EditContext {
        session_idx,
        branch_idx,
        message_idx,
    } = ctx;

    let session = &mut app.sessions[session_idx];
    let old_branch = &session.branches[branch_idx];

    // 1) Copy messages up to the edit point (inclusive)
    let mut new_msgs = old_branch
        .messages
        .iter()
        .take(message_idx + 1)
        .cloned()
        .collect::<Vec<_>>();

    // 2) Append the edited user message on the new branch
    new_msgs.push(Message {
        from: MessageFrom::User,
        content: text.clone(),
    });

    // 3) Create the new branch
    let new_branch_id = session.branches.len();
    session.branches.push(Branch {
        id: new_branch_id,
        name: format!("branch-{new_branch_id}"),
        messages: new_msgs,
    });

    // 4) Switch to the new branch
    session.active_branch = new_branch_id;

    // 5) Start streaming assistant reply on the new branch
    start_streaming_on_branch(app, session_idx, new_branch_id, text)?;

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
            if let Ok(reply) = call_ollama(prompt) {
                let chars: Vec<char> = reply.chars().collect();
                let mut buf = String::new();

                for c in chars {
                    buf.push(c);
                    if buf.len() >= 5 {
                        let _ = tx.send(BackendEvent::AssistantChunk {
                            session_idx,
                            branch_idx,
                            chunk: buf.clone(),
                        });
                        buf.clear();
                        std::thread::sleep(std::time::Duration::from_millis(40));
                    }
                }

                if !buf.is_empty() {
                    let _ = tx.send(BackendEvent::AssistantChunk {
                        session_idx,
                        branch_idx,
                        chunk: buf,
                    });
                }

                let _ = tx.send(BackendEvent::AssistantDone {
                    session_idx,
                    branch_idx,
                });
            }
        });
    }

    Ok(())
}