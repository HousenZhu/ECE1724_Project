mod app;
mod tui;
mod frontend;

use std::io::{stdout, Stdout};
use std::time::Duration;
use std::sync::mpsc;

use anyhow::Result;
use crossterm::{
    event::{self, Event, EnableMouseCapture, DisableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use frontend::keyboard::handle_key_event;
use frontend::mouse::handle_mouse_event;
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::{App, BackendEvent};
use crate::tui::ui as draw_ui;

/// Initialize terminal in raw mode and enter an alternate screen.
fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(out);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore terminal back to normal mode.
fn restore_terminal(mut terminal: Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}

fn main() -> Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = App::new();
    
    // Create a channel for backend events (assistant streaming).
    let (tx, rx) = mpsc::channel::<BackendEvent>();
    app.backend_tx = Some(tx);

    loop {
        // 0) Drain backend events before drawing
        while let Ok(event) = rx.try_recv() {
            match event {
                BackendEvent::AssistantChunk { session_idx, branch_idx, chunk } => {
                    app.append_assistant_chunk(session_idx, branch_idx, chunk);
                }
                BackendEvent::AssistantDone { session_idx, branch_idx, } => {
                    app.finish_streaming(session_idx, branch_idx);
                }

                // BackendEvent::AssistantChunkOnBranch { session_idx, branch_idx, chunk } => {
                //     // New behavior: append chunk on a specific branch
                //     app.append_assistant_chunk_on_branch(session_idx, branch_idx, chunk);
                // }
                // BackendEvent::AssistantDoneOnBranch { session_idx, branch_idx } => {
                //     app.finish_streaming_on_branch(session_idx, branch_idx);
                // }
            }
        }

        // 1) Draw the UI based on current state.
        terminal.draw(|f| draw_ui(f, &mut app))?;

        // 2) Handle input events (non-blocking poll).
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
                    // Delegate key handling to keyboard::handle_key_event.
                    // If it returns true, we should exit the loop.
                    if handle_key_event(key.code, &mut app)? {
                        break;
                    }
                }
                Event::Mouse(m) => {
                    handle_mouse_event(m, &mut app)?;
                }
                _ => {
                    // Ignore other events (e.g. Resize) for now.
                }
            }
        }
    }
    // After breaking out of the loop, restore the terminal and exit cleanly.
    restore_terminal(terminal)?;
    Ok(())
}