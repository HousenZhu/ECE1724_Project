use anyhow::Result;
use crossterm::event::{MouseEvent, MouseEventKind, MouseButton};

use crate::app::App;

/// Width of the left sidebar (sessions panel).
const LEFT_PANEL_WIDTH: u16 = 25;

/// Handle mouse events such as clicking and scrolling.
pub fn handle_mouse_event(me: MouseEvent, app: &mut App) -> Result<()> {
    match me.kind {
        // ===================== LEFT CLICK =====================
        MouseEventKind::Up(MouseButton::Left) => {
            let x = me.column;
            let y = me.row;

            if x < LEFT_PANEL_WIDTH {
                // Left panel

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
            } else {
                // Right panel click (for now we do nothing special).
            }
        }

        // ===================== SCROLL UP ======================
        MouseEventKind::ScrollUp => {
            let x = me.column;

            if x < LEFT_PANEL_WIDTH {
                // Scroll the session list: move active index up.
                if app.active_idx > 0 {
                    app.active_idx -= 1;
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