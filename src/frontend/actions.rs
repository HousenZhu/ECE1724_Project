use std::thread;

use anyhow::Result;

use crate::app::{App, BackendEvent, Message, MessageFrom};

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

/// Send a user message and start a background worker that streams the assistant reply.
pub fn send_user_message_with_streaming(app: &mut App, text: String) -> Result<()> {
    let prompt = text.clone();
    let session_idx = app.active_idx;

    // 1) Append user message to the active session
    {
        let session = &mut app.sessions[session_idx];
        session.messages.push(Message {
            from: MessageFrom::User,
            content: text,
        });
    }

    // 2) Create an empty assistant message for streaming output
    app.start_streaming_assistant(session_idx);

    // 3) Clear the input box
    app.input.clear();

    // 4) Spawn a background worker to call Ollama and stream chunks
    if let Some(tx) = app.backend_tx.clone() {
        thread::spawn(move || {
            // Call Ollama synchronously
            if let Ok(reply) = call_ollama(prompt) {
                // Fake streaming: split reply into small chunks
                let chars: Vec<char> = reply.chars().collect();
                let mut buf = String::new();

                for c in chars {
                    buf.push(c);
                    if buf.len() >= 5 {
                        let _ = tx.send(BackendEvent::AssistantChunk {
                            session_idx,
                            chunk: buf.clone(),
                        });
                        buf.clear();
                        // Small delay to simulate streaming
                        std::thread::sleep(std::time::Duration::from_millis(40));
                    }
                }

                // Flush remaining text
                if !buf.is_empty() {
                    let _ = tx.send(BackendEvent::AssistantChunk {
                        session_idx,
                        chunk: buf,
                    });
                }

                let _ = tx.send(BackendEvent::AssistantDone { session_idx });
            }
        });
    }

    Ok(())
}