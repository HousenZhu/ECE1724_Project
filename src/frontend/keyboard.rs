use anyhow::Result;
use crossterm::event::KeyCode;

use crate::frontend::actions;
use crate::app::{App, InputMode};

/// Handle a single key event.
/// Returns Ok(true) if the app should exit, Ok(false) otherwise.
pub fn handle_key_event(code: KeyCode, app: &mut App) -> Result<bool> {
    match app.input_mode {
        InputMode::Normal => {
            match code {
                // Quit in normal mode.
                KeyCode::Char('q') => {
                    // Tell caller to exit the loop.
                    return Ok(true);
                }

                // New session in normal mode.
                KeyCode::Char('n') => {
                    app.new_session();
                }

                // Move selection up/down using j/k.
                KeyCode::Char('j') => {
                    app.next_session();
                }
                KeyCode::Char('k') => {
                    app.prev_session();
                }

                // Also allow arrow keys in normal mode.
                KeyCode::Up => app.prev_session(),
                KeyCode::Down => app.next_session(),

                // Enter insert mode.
                KeyCode::Char('i') => {
                    app.input_mode = InputMode::Insert;
                }

                // TAB toggles between the button and the list.
                KeyCode::Tab => {
                    app.new_button_selected = !app.new_button_selected;
                }

                KeyCode::Enter => {
                    if app.new_button_selected {
                        // Pressing Enter on the button creates a new session.
                        app.new_session();
                    } else {
                        // Do nothing for now when pressing Enter on the list.
                    }
                }

                _ => {
                    // Ignore other keys in normal mode.
                }
            }
        }

        InputMode::Insert => {
            match code {
                // Leave insert mode and go back to normal.
                KeyCode::Esc => {
                    app.input_mode = InputMode::Normal;
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
                    if !msg.is_empty() {
                        // Clear input first.
                        app.input.clear();

                        // Delegate the actual "send + call ollama" work to the actions module.
                        actions::send_message_via_ollama(app, msg)?;
                    }
                }

                // Allow arrow keys in insert mode too.
                KeyCode::Up => app.prev_session(),
                KeyCode::Down => app.next_session(),

                _ => {}
            }
        }
    }

    Ok(false)
}