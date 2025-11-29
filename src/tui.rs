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
    let msg_area = right_chunks[0];
    // Subtract 2 rows for top/bottom borders of the block.
    let viewport_height = msg_area.height.saturating_sub(2).max(1) as usize;
    let inner_width = msg_area.width.saturating_sub(2) as usize;

    // 1) Build logical lines and capture session title using an immutable borrow to `app`. 
    let (session_title, logical_lines): (String, Vec<(Option<usize>, Line)>) = {
        let active = app.active_session();
        let branch = &active.branches[active.active_branch];

        let mut lines: Vec<(Option<usize>, Line)> = Vec::new();

        for (idx, m) in branch.messages.iter().enumerate() {
            match m.from {
                MessageFrom::Assistant => {
                    // AI on the left
                    let prefix = "AI: ";
                    let raw = m.content.replace("\r\n", "\n");

                    for (i, seg) in raw.split('\n').enumerate() {
                        // first visual line uses "AI: ", following lines are indented
                        let mut current = if i == 0 {
                            format!("{prefix}{seg}")
                        } else {
                            format!("{:width$}{}", "", seg, width = prefix.len())
                        };

                        while current.chars().count() > inner_width {
                            // take one screen-width slice
                            let mut taken = String::new();
                            let mut count = 0;
                            for ch in current.chars() {
                                if count == inner_width {
                                    break;
                                }
                                taken.push(ch);
                                count += 1;
                            }

                            lines.push((None, Line::from(taken)));

                            // remaining part
                            current = current.chars().skip(count).collect();
                            // indent wrapped lines
                            current = format!("{:width$}{}", "", current, width = prefix.len());
                        }

                        lines.push((None, Line::from(current)));
                    }
                }

                MessageFrom::User => {
                    // User on the right: we build left-aligned text first,
                    // then pad with spaces on the left so that it ends near the right edge.
                    let prefix = "You: ";
                    let raw = m.content.replace("\r\n", "\n");
                    let mut first_line = true;

                    for (i, seg) in raw.split('\n').enumerate() {
                        let mut current = if i == 0 {
                            format!("{prefix}{seg}")
                        } else {
                            format!("{:width$}{}", "", seg, width = prefix.len())
                        };

                        while current.chars().count() > inner_width {
                            let mut taken = String::new();
                            let mut count = 0;
                            for ch in current.chars() {
                                if count == inner_width {
                                    break;
                                }
                                taken.push(ch);
                                count += 1;
                            }

                            // right-align this visual line
                            let len = taken.chars().count();
                            let padding = inner_width.saturating_sub(len);
                            let padded = format!("{}{}", " ".repeat(padding), taken);

                            // Only the very first visual line of this user message
                            // is tagged with Some(idx) for hitbox detection.
                            let owner = if first_line { Some(idx) } else { None };
                            first_line = false;

                            lines.push((owner, Line::from(padded)));

                            current = current.chars().skip(count).collect();
                            current = format!("{:width$}{}", "", current, width = prefix.len());
                        }

                        // last fragment (shorter than inner_width)
                        let len = current.chars().count();
                        let padding = inner_width.saturating_sub(len);
                        let padded = format!("{}{}", " ".repeat(padding), current);

                        let owner = if first_line { Some(idx) } else { None };
                        first_line = false;

                        lines.push((owner, Line::from(padded)));
                    }
                }
            }

            // spacer line after each message
            lines.push((None, Line::from("")));
        }

        (active.title.clone(), lines)
    };

    // 2) Now that the immutable borrow is gone, we can safely mutate `app.user_msg_hitboxes`.
    app.user_msg_hitboxes.clear();

    // Clamp scroll offset so we never scroll beyond the end.
    let total_lines = logical_lines.len();
    let max_scroll = total_lines.saturating_sub(viewport_height);
    let scroll = app.msg_scroll.min(max_scroll);

    // Take the visible window of lines and record hitboxes for user messages.
    let mut visible_lines: Vec<Line> = Vec::new();

    for (line_i, (owner, line)) in logical_lines.into_iter().enumerate() {
        if line_i < scroll || line_i >= scroll + viewport_height {
            continue;
        }

        // Compute terminal y coordinate for this logical line.
        let screen_y = msg_area.y + 1 + (line_i - scroll) as u16;

        // If this line belongs to a user message, record a hitbox so the mouse handler can detect hover/click.
        if let Some(msg_idx) = owner {
            let rect = Rect {
                x: msg_area.x,
                y: screen_y,
                width: msg_area.width,
                height: 1,
            };
            app.user_msg_hitboxes.push((msg_idx, rect));
        }

        visible_lines.push(line);
    }

    // Render the messages paragraph.
    let messages_widget = Paragraph::new(visible_lines)
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::RIGHT)
                .title(session_title),
        );
    
    f.render_widget(messages_widget, msg_area);

    // If a user message is hovered, render a small "edit" label on the right side of that message line.
    if let Some(msg_idx) = app.hovered_user_msg {
        if let Some((_, rect)) = app
            .user_msg_hitboxes
            .iter()
            .find(|(idx, _)| *idx == msg_idx)
        {
            let edit_width = 6;
            let edit_rect = Rect {
                x: rect.x + rect.width.saturating_sub(edit_width + 1),
                y: rect.y + 1,
                width: edit_width,
                height: 1,
            };

            let edit_widget = Paragraph::new("edit").alignment(Alignment::Center);
            f.render_widget(edit_widget, edit_rect);
        }
    }

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