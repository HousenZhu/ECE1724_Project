use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Span, Line},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
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
        .block(Block::default().borders(Borders::LEFT | Borders::TOP).title("Actions"));

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
        .block(Block::default().borders(Borders::LEFT | Borders::BOTTOM).title("Sessions"))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");
    
    f.render_stateful_widget(sessions_list, left_chunks[1], &mut app.list_state);

    // ===== Right: top messages + bottom input =====
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),       // Messages
            Constraint::Length(5)])   // Input
        .split(right_panel);

    // Messages area with scroll support.
    let active = app.active_session();

    // 1) Determine how many lines can be shown in the messages area.
    let msg_area = right_chunks[0];
    let viewport_height = msg_area.height.max(1) as usize;
    let inner_width = msg_area.width.saturating_sub(2) as usize;

    // 2) Build all message lines.
    let mut lines: Vec<Line> = Vec::new();

    for m in &active.messages {
        match m.from {
            MessageFrom::Assistant => {
                // AI: left side
                let text = format!("AI: {}", m.content);
                lines.push(Line::from(vec![
                    Span::styled(text, Style::default()),
                ]));
            }
            MessageFrom::User => {
                // You on the right: pad spaces so the text appears at the right edge.
                let base = format!("You: {}", m.content);

                let len = base.chars().count();
                let padding = inner_width.saturating_sub(len);

                let padded = format!("{}{}", " ".repeat(padding), base);

                lines.push(Line::from(vec![
                    Span::styled(padded, Style::default()),
                ]));
            }
        }
    }


    // 3) Clamp scroll offset so we never scroll beyond the end.
    let total_lines = lines.len();
    let max_scroll = total_lines.saturating_sub(viewport_height);
    let scroll = app.msg_scroll.min(max_scroll);

    // 4) Take the visible window of lines.
    let visible_lines: Vec<Line> = if total_lines == 0 {
        Vec::new()
    } else {
        lines
            .into_iter()
            .skip(scroll)
            .take(viewport_height)
            .collect::<Vec<Line>>()
    };

    // 5) Join lines into a single string for Paragraph.
    let messages_widget = Paragraph::new(visible_lines)
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::RIGHT)
                .title(active.title.clone())
        )
        .wrap(Wrap { trim: false });
    f.render_widget(messages_widget, msg_area);

    // ===== Bottom input area (input + send button) =====
    let input_area = right_chunks[1];

    // Determine the label for input mode (NORMAL / INSERT)
    let mode_label = match app.input_mode {
        InputMode::Normal => "[NORMAL]",
        InputMode::Insert => "[INSERT]",
    };
    let input_title = format!("Input {}", mode_label);

    // reset the send button area every frame.
    // It will be set again below.
    app.send_button_area = None;

    // Split the bottom area horizontally:
    // - Left: main text input
    // - Right: a small fixed-width area for the send button
    // let input_chunks = Layout::default()
    //     .direction(Direction::Horizontal)
    //     .constraints([
    //         Constraint::Min(0),     // Input takes all remaining space
    //         Constraint::Length(8),  // Fixed width for send button
    //     ])
    //     .split(input_area);

    // let input_box = input_chunks[0];
    // let button_box = input_chunks[1];

    // // 1) Render the input box.
    // //    Users type messages here in INSERT mode.
    // let input_widget = Paragraph::new(app.input.as_str())
    //     .block(
    //         Block::default()
    //             .borders(Borders::BOTTOM | Borders::RIGHT)
    //             .title(input_title),
    //     );
    // f.render_widget(input_widget, input_area);

    // // 2) Render the send button on the right
    // //    This is purely visual for now; actual behavior is handled
    // //    in the key/mouse event handlers.
    // let send_button = Paragraph::new("▶ Send")
    //     .alignment(Alignment::Center)
    //     .block(
    //         Block::default()
    //             .borders(Borders::BOTTOM | Borders::RIGHT)
    //     );
    // f.render_widget(send_button, button_box);
    // let mode_label = match app.input_mode {
    //     InputMode::Normal => "[NORMAL]",
    //     InputMode::Insert => "[INSERT]",
    // };

    // let input_title = format!("Input {}", mode_label);

    // let input_widget = Paragraph::new(app.input.as_str())
    //     .block(Block::default().borders(Borders::BOTTOM | Borders::RIGHT).title(input_title));
    // f.render_widget(input_widget, right_chunks[1]);
    
    // 1) Render the full-width input box at the bottom.
    let input_widget = Paragraph::new(app.input.as_str())
        .block(
            Block::default()
                .borders(Borders::BOTTOM | Borders::RIGHT)
                .title(input_title),
        );
    f.render_widget(input_widget, input_area);

    // 2) Overlay a small "send" icon near the bottom-right corner of the input area.
    let icon_width = 9;   
    let icon_height = 3;  

    // Position the icon slightly inside the bottom-right border
    let icon_x = input_area.x + input_area.width.saturating_sub(icon_width + 1);
    let icon_y = input_area.y + input_area.height.saturating_sub(icon_height + 1);

    let icon_rect = Rect::new(icon_x, icon_y, icon_width, icon_height);

    // Save this rect into the app so the mouse handler can use it.
    app.send_button_area = Some(icon_rect);

    // Render a bordered block with a Unicode send icon
    let send_button = Paragraph::new("➤ Send")
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
        );

    f.render_widget(send_button, icon_rect);
}