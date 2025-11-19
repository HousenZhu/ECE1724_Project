mod app;
mod tui;

use std::io::{stdout, Stdout};
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::App;
use crate::tui::ui as draw_ui;

use serde::Deserialize;

/// Response format for Ollama's /api/generate endpoint.
#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

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

/// Initialize terminal in raw mode and enter an alternate screen.
fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(out);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore terminal back to normal mode.
fn restore_terminal(mut terminal: Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn main() -> Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = App::new();

    loop {
        // 1) Draw the UI based on current state.
        terminal.draw(|f| draw_ui(f, &mut app))?;

        // 2) Handle input events (non-blocking poll).
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    
                    // Ctrl+Q to quit the application.
                    KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        break;
                    }

                    // Switch sessions with arrow keys.
                    KeyCode::Up => app.prev_session(),
                    KeyCode::Down => app.next_session(),

                    // Ctrl+N to create a new session.
                    KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // Create a new session and switch to it.
                        app.new_session();
                    }

                    // Handle text input.
                    KeyCode::Char(c) => {
                        app.input.push(c);
                    }
                    KeyCode::Backspace => {
                        app.input.pop();
                    }

                    // On Enter: send the user message and call Ollama for a response.
                    KeyCode::Enter => {
                        let msg = app.input.trim().to_string();
                        // Add user message first.
                        if !msg.is_empty() {
                            app.push_user_message(msg.clone());
                            app.input.clear();

                            // Call Ollama
                            match call_ollama(&msg) {
                                Ok(reply) => {
                                    app.push_assistant_message(reply);
                                }
                                Err(err) => {
                                    app.push_assistant_message(format!(
                                        "Error calling Ollama: {}", err
                                    ));
                                }
                            }
                        }
                    }

                    _ => {}
                }
            }
        }
    }

    restore_terminal(terminal)?;
    Ok(())
}