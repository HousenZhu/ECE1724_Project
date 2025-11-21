use anyhow::Result;
use crossterm::{
    event::{MouseEvent, MouseEventKind, MouseButton},
};
use ratatui::layout::Rect;

use crate::app::App;
use crate::frontend::actions;

/// Width of the left sidebar (sessions panel).
const LEFT_PANEL_WIDTH: u16 = 25;
// const SESSION_LIST_ROW_START: u16 = 4;
// /// Size of the send button rectangle (must match tui.rs).
// const SEND_BUTTON_WIDTH: u16 = 7;
// const SEND_BUTTON_HEIGHT: u16 = 3;

/// Handle mouse events such as clicking and scrolling.
pub fn handle_mouse_event(me: MouseEvent, app: &mut App) -> Result<()> {
    match me.kind {
        // ===================== LEFT CLICK =====================
        MouseEventKind::Up(MouseButton::Left) => {
            let x = me.column;
            let y = me.row;

            // Left panel
            if x < LEFT_PANEL_WIDTH {
                // 1) New Session button area = top 3 rows.
                if y < 3 {
                    app.new_session();
                    app.new_button_selected = true;
                    return Ok(());
                }

                // 2) Session list area starts at row 4.
                let list_y = y as usize - 4;
                if list_y < app.sessions.len() {
                    app.active_idx = list_y;
                    app.new_button_selected = false;
                }
                return Ok(());
            } 
            
            // Right panel: check send button --------
            if let Some(area) = app.send_button_area {
                if point_in_rect(x, y, area) {
                    // Click is inside the send button
                    let msg = app.input.trim().to_string();
                    if !msg.is_empty() {
                        // Clear input immediately
                        app.input.clear();
                        // Use the same sending logic as Enter
                        actions::send_message_via_ollama(app, msg)?;
                    }
                }
            }
        }

        // ===================== SCROLL UP ======================
        MouseEventKind::ScrollUp => {
            let x = me.column;

            if x < LEFT_PANEL_WIDTH {
                // Scroll the session list: move active index up.
                if app.active_idx > 0 {
                    app.active_idx -= 1;
                    app.list_state.select(Some(app.active_idx));
                }
            } else {
                // Scroll the message area up.
                if app.msg_scroll > 0 {
                    app.msg_scroll -= 1;
                }
            }
        }

        // ===================== SCROLL DOWN ====================
        MouseEventKind::ScrollDown => {
            let x = me.column;

            if x < LEFT_PANEL_WIDTH {
                // Scroll the session list: move active index down.
                if app.active_idx + 1 < app.sessions.len() {
                    app.active_idx += 1;
                }
            } else {
                // Scroll the message area down (clamped in rendering).
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