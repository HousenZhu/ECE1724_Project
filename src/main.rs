mod app;
mod tui;
mod frontend;

use std::io::{stdout, Stdout};
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use frontend::keyboard::handle_key_event;
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::{App};
use crate::tui::ui as draw_ui;

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
                // Delegate key handling to keyboard::handle_key_event.
                // If it returns true, we should exit the loop.
                if handle_key_event(key.code, &mut app)? {
                    break;
                }
            }
        }
    }
    // After breaking out of the loop, restore the terminal and exit cleanly.
    restore_terminal(terminal)?;
    Ok(())
}