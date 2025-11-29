use anyhow::Result;
use crossterm::event::KeyCode;

use crate::frontend::actions;
use crate::app::{App, InputMode, MessageFrom, EditContext};

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

                KeyCode::Char('[') => { app.prev_branch(); }
                KeyCode::Char(']') => { app.next_branch(); }

                KeyCode::Enter => {
                    if app.new_button_selected {
                        // Pressing Enter on the button creates a new session.
                        app.new_session();
                    } else {
                        // Do nothing for now when pressing Enter on the list.
                    }
                }

                KeyCode::Char('e') => {
                    // Get the active session and branch
                    let session_idx = app.active_idx;
                    let session = &app.sessions[session_idx];
                    let branch_idx = session.active_branch;
                    let branch = &session.branches[branch_idx];

                    // Find the most recent user message
                    if let Some((msg_idx, last_user)) = branch
                        .messages
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, m)| matches!(m.from, MessageFrom::User))
                    {
                        // Load the message content into the input box
                        app.input.clear();
                        app.input.push_str(&last_user.content);

                        // Save edit context: editing will fork a new branch
                        app.edit_ctx = Some(EditContext {
                            session_idx,
                            branch_idx,
                            message_idx: msg_idx,
                        });

                        // Switch to INSERT mode so the user can modify the message
                        app.input_mode = InputMode::Insert;
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
                    if msg.is_empty() {
                        return Ok(false);
                    }

                    // Clear input first.
                    app.input.clear();

                    if let Some(ctx) = app.edit_ctx.take() {
                        // We are editing an existing user message.
                        // This will fork a new branch and overwrite that message there.
                        actions::fork_and_send_from_edit(app, ctx, msg)?;
                    } else {
                        // Normal case: send a brand new user message on the active branch.
                        actions::send_user_message_with_streaming(app, msg)?;
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