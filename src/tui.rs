use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{App, MessageFrom, InputMode};

/// Draw the whole UI based on the current App state.
pub fn ui(f: &mut Frame, app: &mut App) {
    // Split the screen into left (sessions) and right (chat).
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(25), Constraint::Min(0)])
        .split(f.area());
    
    let left_panel = main_chunks[0];
    let right_panel = main_chunks[1];

    // ===== Left: session list =====
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // For the button
            Constraint::Min(0),     // For session list
        ])
        .split(left_panel);

    // 1) Render the "New Session" button
    let new_session_label = if app.new_button_selected {
        // Highlight when focused
        "▶ [ New Session ]"
    } else {
        "  [ New Session ]"
    };

    let new_session_widget = Paragraph::new(new_session_label)
        .block(Block::default().borders(Borders::ALL).title("Actions"));

    f.render_widget(new_session_widget, left_chunks[0]);

    // 2) Render session list BELOW the button
    let items: Vec<ListItem> = app
        .sessions
        .iter()
        .map(|s| {
            // Display both the session ID and title.
            let label = format!("[{}] {}", &s.id[..4], s.title);
            ListItem::new(Span::raw(label))
    })
        .collect();

    let sessions_list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Sessions"))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");
    
    f.render_stateful_widget(sessions_list, left_chunks[1], &mut app.list_state);

    // ===== Right: top messages + bottom input =====
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),       // Messages
            Constraint::Length(3)])   // Input
        .split(right_panel);

    // Messages area.
    let active = app.active_session();
    let mut text = String::new();
    for m in &active.messages {
        let who = match m.from {
            MessageFrom::User => "You",
            MessageFrom::Assistant => "AI",
        };
        text.push_str(&format!("{who}: {}\n", m.content));
    }

    let messages_widget = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(active.title.clone()));
    f.render_widget(messages_widget, right_chunks[0]);

    // Input area.
    let mode_label = match app.input_mode {
        InputMode::Normal => "[NORMAL]",
        InputMode::Insert => "[INSERT]",
    };

    let input_title = format!("Input {}", mode_label);

    let input_widget = Paragraph::new(app.input.as_str())
        .block(Block::default().borders(Borders::ALL).title(input_title));
    f.render_widget(input_widget, right_chunks[1]);
}