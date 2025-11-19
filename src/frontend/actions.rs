use anyhow::Result;

use crate::app::App;

use serde::Deserialize;

/// Response format for Ollama's /api/generate endpoint.
#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

/// High-level action: send a user message and call the Ollama backend.
/// This function mutates the App state by appending both user and assistant messages.
pub fn send_message_via_ollama(app: &mut App, text: String) -> Result<()> {
    // 1) First, store the user message in the active session.
    app.push_user_message(text.clone());

    // 2) Call Ollama.
    let reply = call_ollama(&text)?;

    // 3) Store the assistant reply in the active session.
    app.push_assistant_message(reply);

    Ok(())
}

/// Low-level helper that actually talks to the Ollama server.
/// Call Ollama's local LLM through the HTTP API.
/// This version is synchronous and blocks until the model finishes.
fn call_ollama(prompt: &str) -> anyhow::Result<String> {
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