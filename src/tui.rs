use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect, Margin},
    style::{Modifier, Style},
    text::{Span, Line},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use unicode_width::UnicodeWidthStr;

use crate::app::{App, MessageFrom, InputMode};

/// Draw the whole UI based on the current App state.
pub fn ui(f: &mut Frame, app: &mut App) {
    // Split the screen into left (sessions) and right (chat).
    let sidebar_width = app.sidebar_width();

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(sidebar_width), Constraint::Min(0)])
        .split(f.area());
    
    let left_panel = main_chunks[0];
    let right_panel = main_chunks[1];

    let input_min_height: u16 = 3; // minimum rows for input
    let input_max_height: u16 = 10; // maximum rows for input

    // Reserve some width on the right for the send button (icon + padding).
    let reserved_for_button: u16 = 9;

    // Inner width for text wrapping inside the input box (without borders and button area).
    let input_inner_width = right_panel
        .width
        .saturating_sub(2)                  // remove left/right borders
        .saturating_sub(reserved_for_button) as usize;

    let mut input_lines = 1usize;
    if input_inner_width > 0 && !app.input.is_empty() {
        let text = app.input.replace("\r\n", "\n");
        input_lines = text
            .split('\n')
            .map(|line| {
                let len = line.chars().count();
                if len == 0 {
                    1
                } else {
                    // number of wrapped lines for this logical line
                    (len - 1) / input_inner_width + 1
                }
            })
            .sum();
    }
    
    let input_height =
        input_lines.clamp(input_min_height as usize, input_max_height as usize) as u16 + 2;
        
    let toggle_sidebar_icon = "☰";
    let toggle_sidebar_icon_w = UnicodeWidthStr::width(toggle_sidebar_icon) as u16;

    // ===== Left: session list =====
    if app.sidebar_collapsed {        
        // collapsed bar
        let bar_block = Block::default().borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM);
        f.render_widget(bar_block, left_panel);

        // clickable icon cell
        let toggle_render_rect = Rect::new(
            left_panel.x + 1, 
            left_panel.y + 1, 
            toggle_sidebar_icon_w.max(1), 
            1,
        );
        
        f.render_widget(Paragraph::new(toggle_sidebar_icon), toggle_render_rect);

        let toggle_hitbox = Rect::new(
            toggle_render_rect.x,            
            toggle_render_rect.y,                 
            2,         
            1,
        );
        app.toggle_sidebar_area = Some(toggle_hitbox);

    } else {
        // Expanded: top header (toggle + New Chat) + session list.
        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Header row (toggle + New Chat)
                Constraint::Min(0),     // Session list
            ])
            .split(left_panel);

        let header_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(3),  // toggle area
                Constraint::Min(0),     // new chat area
            ])
            .split(left_chunks[0]);

        let toggle_r = header_chunks[0];
        let inner_toggle_sidebar_icon_w = toggle_r.width.saturating_sub(2); // remove borders
        let toggle_start_x = toggle_r.x + 2 + inner_toggle_sidebar_icon_w.saturating_sub(toggle_sidebar_icon_w) / 2;

        app.toggle_sidebar_area = Some(Rect::new(
            toggle_start_x,
            toggle_r.y + 1, 
            2,
            1,
        ));

        let toggle_widget = Paragraph::new(toggle_sidebar_icon)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::TOP | Borders::LEFT)
            );
        f.render_widget(toggle_widget, toggle_r);

        let new_chat_icon = "[✚ New]";

        // Use a tight clickable area inside the right header block
        let r = header_chunks[1];
        let w = UnicodeWidthStr::width(new_chat_icon) as u16;

        let inner_w = r.width.saturating_sub(2); // remove borders
        let start_x = r.x + 2 + inner_w.saturating_sub(w) / 2;

        app.new_chat_area = Some(Rect::new(
            start_x,
            r.y + 1, 
            w,
            1,
        ));

        let new_chat_widget = Paragraph::new(new_chat_icon)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::TOP | Borders::RIGHT)
                    .title("Chats"),
            );
        f.render_widget(new_chat_widget, header_chunks[1]);

        let items: Vec<ListItem> = app
            .sessions
            .iter()
            .map(|s| {
                let label = format!("[{}] {}", &s.id[..4], s.title);
                ListItem::new(Span::raw(label))
            })
            .collect();

        let sessions_list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::BOTTOM | Borders::RIGHT)
                    .title("Sessions"),
            )
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .highlight_symbol("> ");

        f.render_stateful_widget(sessions_list, left_chunks[1], &mut app.list_state);

        app.session_hitboxes.clear();

        let list_outer = left_chunks[1];
        let list_inner = list_outer.inner(Margin { vertical: 1, horizontal: 1 }); // exclude borders

        for (i, s) in app.sessions.iter().enumerate() {
            // Must match exactly what you show in the list
            let label = format!("[{}] {}", &s.id[..4], s.title);

            let w = UnicodeWidthStr::width(label.as_str()) as u16;
            let w = w.min(list_inner.width.max(1));

            // Each list item is 1 row tall
            let y = list_inner.y + i as u16;

            // Only create a hitbox if it fits inside the visible list area
            if y < list_inner.y + list_inner.height {
                app.session_hitboxes.push((i, Rect::new(list_inner.x, y, w, 1)));
            }
        }
    }

    // ===== Right: top messages + bottom input =====
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),                  // Messages
            Constraint::Length(input_height)])   // Input auto height
        .split(right_panel);

    // Messages area with scroll support.
    let msg_area = right_chunks[0];
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
                            // indent 1ped lines
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

    // 2) mutate `app.user_msg_hitboxes`.
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

    app.input_area = Some(input_area);

    // Determine the label for input mode (NORMAL / INSERT)
    let mode_label = match app.input_mode {
        InputMode::Normal => "[NORMAL]",
        InputMode::Insert => "[INSERT]",
    };
    let input_title = format!("Input {}", mode_label);

    // reset the send button area every frame.
    app.send_button_area = None;
    
    // 1) Render the full-width input box at the bottom.
    // Manually wrap the input text into visual lines, using the inner width of the input box.
    let reserved_for_button: u16 = 11; // must match the value used above
    let input_inner_width = input_area
        .width
        .saturating_sub(2)                      // borders
        .saturating_sub(reserved_for_button) as usize;

    let mut input_visual_lines: Vec<Line> = Vec::new();

    if input_inner_width == 0 || app.input.is_empty() {
        input_visual_lines.push(Line::from(app.input.as_str()));
    } else {
        let raw = app.input.replace("\r\n", "\n");

        for seg in raw.split('\n') {
            let mut current = seg.to_string();

            if current.is_empty() {
                input_visual_lines.push(Line::from(""));
                continue;
            }

            while current.chars().count() > input_inner_width {
                let mut taken = String::new();
                let mut count = 0;

                for ch in current.chars() {
                    if count == input_inner_width {
                        break;
                    }
                    taken.push(ch);
                    count += 1;
                }

                input_visual_lines.push(Line::from(taken));
                current = current.chars().skip(count).collect();
            }

            input_visual_lines.push(Line::from(current));
        }
    }

    // 2) Always show the last N lines so the cursor area stays visible.
    let total_lines = input_visual_lines.len().max(1);
    let inner_height = input_area.height.saturating_sub(2).max(1) as usize; // minus borders
    let visible_lines = inner_height;

    // Maximum offset you can scroll up from the bottom.
    let max_offset = total_lines.saturating_sub(visible_lines);
    if app.input_scroll > max_offset {
        app.input_scroll = max_offset;
    }

    let offset_from_bottom = app.input_scroll;
    let start = total_lines
        .saturating_sub(visible_lines + offset_from_bottom)
        .max(0);

    let visible_input: Vec<Line> = input_visual_lines
        .into_iter()
        .skip(start)
        .take(visible_lines)
        .collect();

    let input_widget = Paragraph::new(visible_input)
        .block(
            Block::default()
                .borders(Borders::BOTTOM | Borders::RIGHT)
                .title(input_title),
        );

    f.render_widget(input_widget, input_area);

    // 3) Overlay a small "send" icon near the bottom-right corner of the input area.
    let base_icon_width: u16 = 9;
    let base_icon_height: u16 = 3;

    let icon_width = base_icon_width.min(input_area.width.max(1));
    let icon_height = base_icon_height.min(input_area.height.max(1));

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