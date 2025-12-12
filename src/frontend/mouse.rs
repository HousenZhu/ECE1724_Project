use anyhow::Result;
use crossterm::{
    event::{MouseEvent, MouseEventKind, MouseButton},
};
use ratatui::layout::Rect;

use crate::app::{App, EditContext, InputMode};
use crate::frontend::actions;

// const SESSION_LIST_ROW_START: u16 = 4;
// /// Size of the send button rectangle (must match tui.rs).
// const SEND_BUTTON_WIDTH: u16 = 7;
// const SEND_BUTTON_HEIGHT: u16 = 3;

/// Handle mouse events such as clicking and scrolling.
pub fn handle_mouse_event(me: MouseEvent, app: &mut App) -> Result<()> {
    match me.kind {
        MouseEventKind::Moved => {
            // Mouse hover detection
            let x = me.column;
            let y = me.row;

            let mut hovered = None;

            for (msg_idx, rect) in &app.user_msg_hitboxes {
                if point_in_rect(x, y, *rect) {
                    hovered = Some(*msg_idx);
                    break;
                }
            }

            app.hovered_user_msg = hovered;
        }
        // LEFT CLICK
        MouseEventKind::Up(MouseButton::Left) => {
            let x = me.column;
            let y = me.row;

            let sidebar_width = app.sidebar_width();
            // ===================== LEFT PANEL =====================
            // Click inside left sidebar region
            if x < sidebar_width {
                if app.sidebar_collapsed {
                    // In collapsed mode, any click on the thin bar toggles it.
                    app.toggle_sidebar();
                    return Ok(());
                }

                // Expanded mode:
                // Click precise buttons instead of a whole header row
                if let Some(r) = app.toggle_sidebar_area {
                    if point_in_rect(x, y, r) {
                        app.toggle_sidebar();
                        return Ok(());
                    }
                }

                if let Some(r) = app.new_chat_area {
                    if point_in_rect(x, y, r) {
                        app.new_session();
                        app.new_button_selected = true;
                        return Ok(());
                    }
                }

                // Click on a session label (hitbox-based)
                if let Some((idx, _)) = app
                    .session_hitboxes
                    .iter()
                    .find(|(_, r)| point_in_rect(x, y, *r))
                {
                    app.active_idx = *idx;
                    app.list_state.select(Some(app.active_idx));
                    app.new_button_selected = false;
                }

                return Ok(());
            }
            // ===================== RIGHT PANEL =====================

            // 1) Check if the click is inside the send button area.
            if let Some(area) = app.send_button_area {
                if point_in_rect(x, y, area) {
                    // Click is inside the send button.
                    let msg = app.input.trim().to_string();
                    if !msg.is_empty() {
                        // Let the actions module handle sending + streaming.
                        actions::send_user_message_with_streaming(app, msg)?;
                    }
                    return Ok(());
                }
            }

            // 2) Check if the click is on a user message line (= edit / fork).
            if let Some((msg_idx, _rect)) = app
                .user_msg_hitboxes
                .iter()
                .find(|(_, r)| point_in_rect(x, y, *r))
            {
                let session_idx = app.active_idx;
                let branch_idx = app.sessions[session_idx].active_branch;

                // Fetch the original user message.
                let msg = &app.sessions[session_idx]
                    .branches[branch_idx]
                    .messages[*msg_idx];

                // Pre-fill the input box with the message content.
                app.input = msg.content.clone();
                app.input_mode = InputMode::Insert;

                // Store edit context so that pressing Enter will fork a new branch
                // starting from this message.
                app.edit_ctx = Some(EditContext {
                    session_idx,
                    branch_idx,
                    message_idx: *msg_idx,
                });

                return Ok(());
            }
        }

        // SCROLL UP
        MouseEventKind::ScrollUp => {
            let x = me.column;
            let y = me.row;
            let sidebar_width = app.sidebar_width();

            if x < sidebar_width {
                // Scroll the session list: move active index up.
                if app.active_idx > 0 {
                    app.active_idx -= 1;
                    app.list_state.select(Some(app.active_idx));
                }
            } else if let Some(area) = app.input_area {
                // If mouse is inside the input area, scroll the input box.
                if y >= area.y && y < area.y + area.height {
                    // Move view further up in the input (offset from bottom).
                    app.input_scroll = app.input_scroll.saturating_add(1);
                } else if app.msg_scroll > 0 {
                    // Otherwise scroll the message area up.
                    app.msg_scroll -= 1;
                }
            } else if app.msg_scroll > 0 {
                // Fallback: if we don't yet have an input rect, just scroll messages.
                app.msg_scroll -= 1;
            }
        }

        // SCROLL DOWN
        MouseEventKind::ScrollDown => {
            let x = me.column;
            let y = me.row;
            let sidebar_width = app.sidebar_width();

            if x < sidebar_width {
                // Scroll the session list: move active index down.
                if app.active_idx + 1 < app.sessions.len() {
                    app.active_idx += 1;
                    app.list_state.select(Some(app.active_idx));
                }
            } else if let Some(area) = app.input_area {
                if y >= area.y && y < area.y + area.height {
                    // Scroll input back towards the bottom.
                    if app.input_scroll > 0 {
                        app.input_scroll -= 1;
                    }
                } else {
                    // Scroll the message area down (clamped in rendering).
                    app.msg_scroll += 1;
                }
            } else {
                // Fallback: scroll messages.
                app.msg_scroll += 1;
            }
        }

        _ => {}
    }

    Ok(())
}

/// Helper: check whether mouse coordinate is inside a Rect.
fn point_in_rect(x: u16, y: u16, rect: Rect) -> bool {
    x >= rect.x
        && x < rect.x + rect.width
        && y >= rect.y
        && y < rect.y + rect.height
}