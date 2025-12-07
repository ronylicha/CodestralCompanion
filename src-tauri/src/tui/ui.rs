use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};
use crate::tui::app::App;
use crate::tui::logo::{MISTRAL_ICON, MISTRAL_COLOR};
use crate::chat::ChatMode;

const MAX_TOKENS: usize = 32000;

pub fn draw(frame: &mut Frame, app: &App) {
    let size = frame.area();
    
    // Main layout: Header | Chat | Input | Status
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // Header
            Constraint::Min(10),    // Chat area
            Constraint::Length(3),  // Input
            Constraint::Length(1),  // Status bar
        ])
        .split(size);

    draw_header(frame, app, chunks[0]);
    draw_chat(frame, app, chunks[1]);
    draw_input(frame, app, chunks[2]);
    draw_status_bar(frame, app, chunks[3]);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let header_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(8),  // Icon
            Constraint::Min(20),    // Title and path
        ])
        .split(area);

    // Draw icon
    let icon_lines: Vec<Line> = MISTRAL_ICON.iter()
        .map(|line| Line::from(Span::styled(*line, Style::default().fg(MISTRAL_COLOR))))
        .collect();
    let icon = Paragraph::new(icon_lines);
    frame.render_widget(icon, header_layout[0]);

    // Draw title and path
    let title_text = vec![
        Line::from(vec![
            Span::styled("Codestral", Style::default().fg(MISTRAL_COLOR).add_modifier(Modifier::BOLD)),
            Span::raw(" Companion "),
            Span::styled("v0.1.0", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("📁 ", Style::default()),
            Span::styled(
                app.project_path.to_string_lossy().to_string(),
                Style::default().fg(Color::Cyan),
            ),
        ]),
    ];
    let title = Paragraph::new(title_text);
    frame.render_widget(title, header_layout[1]);
}

fn draw_chat(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.messages.is_empty() {
        let welcome = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Bienvenue! Tapez votre question ou instruction ci-dessous.",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Alt+M: changer de mode | /: commandes | ↑↓: historique",
                Style::default().fg(Color::DarkGray),
            )),
        ]);
        frame.render_widget(welcome, inner);
        return;
    }

    let mut items: Vec<ListItem> = Vec::new();
    
    for msg in &app.messages {
        let (prefix, style) = if msg.is_user {
            ("> ", Style::default().fg(Color::Cyan))
        } else {
            ("● ", Style::default().fg(Color::Green))
        };
        
        // Wrap content to fit area
        let content_lines: Vec<Line> = msg.content
            .lines()
            .flat_map(|line| {
                if line.is_empty() {
                    vec![Line::from("")]
                } else {
                    // Simple word wrapping
                    let max_width = (inner.width as usize).saturating_sub(4);
                    wrap_line(line, max_width)
                }
            })
            .collect();

        // First line with prefix
        if let Some(first) = content_lines.first() {
            let mut spans = vec![Span::styled(prefix, style)];
            spans.extend(first.spans.clone());
            items.push(ListItem::new(Line::from(spans)));
        }

        // Remaining lines indented
        for line in content_lines.iter().skip(1) {
            let mut spans = vec![Span::raw("  ")];
            spans.extend(line.spans.clone());
            items.push(ListItem::new(Line::from(spans)));
        }

        // Empty line between messages
        items.push(ListItem::new(Line::from("")));
    }

    // Loading indicator
    if app.loading {
        items.push(ListItem::new(Line::from(vec![
            Span::styled("● ", Style::default().fg(Color::Yellow)),
            Span::styled("Réflexion en cours...", Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC)),
        ])));
    }

    // Calculate scroll
    let total_items = items.len();
    let visible_height = inner.height as usize;
    let scroll_offset = if total_items > visible_height {
        (total_items - visible_height).min(app.scroll as usize)
    } else {
        0
    };

    let list = List::new(items);
    frame.render_widget(list, inner);

    // Scrollbar
    if total_items > visible_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        let mut scrollbar_state = ScrollbarState::new(total_items)
            .position(scroll_offset);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(" Input ", Style::default().fg(Color::Cyan)));

    let input_area = input_block.inner(area);
    frame.render_widget(input_block, area);

    // Build input text with cursor
    let before_cursor = &app.input[..app.cursor_pos];
    let cursor_char = app.input.get(app.cursor_pos..app.cursor_pos + 1).unwrap_or(" ");
    let after_cursor = &app.input[app.cursor_pos.saturating_add(1).min(app.input.len())..];

    let input_line = Line::from(vec![
        Span::raw("> "),
        Span::raw(before_cursor),
        Span::styled(cursor_char, Style::default().bg(Color::White).fg(Color::Black)),
        Span::raw(after_cursor),
    ]);

    let input = Paragraph::new(input_line);
    frame.render_widget(input, input_area);
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mode_style = match app.mode {
        ChatMode::Ask => Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
        ChatMode::Plan => Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ChatMode::Code => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        ChatMode::Auto => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    };

    let mode_name = match app.mode {
        ChatMode::Ask => "ASK",
        ChatMode::Plan => "PLAN",
        ChatMode::Code => "CODE",
        ChatMode::Auto => "AUTO",
    };

    let remaining_pct = ((MAX_TOKENS.saturating_sub(app.tokens)) * 100) / MAX_TOKENS;
    
    let status = Line::from(vec![
        Span::styled(" -- ", Style::default().fg(Color::DarkGray)),
        Span::styled(mode_name, mode_style),
        Span::styled(" [Alt+M] ", Style::default().fg(Color::DarkGray)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::raw(format!("{} tok", app.tokens)),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::raw(format!("~{}%", remaining_pct)),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled("/: menu", Style::default().fg(Color::DarkGray)),
    ]);

    let status_bar = Paragraph::new(status)
        .style(Style::default().bg(Color::Rgb(30, 30, 30)));
    
    frame.render_widget(status_bar, area);
}

fn wrap_line(line: &str, max_width: usize) -> Vec<Line<'static>> {
    if line.len() <= max_width {
        return vec![Line::from(line.to_string())];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    
    for word in line.split_whitespace() {
        if current.is_empty() {
            current = word.to_string();
        } else if current.len() + 1 + word.len() <= max_width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(Line::from(current));
            current = word.to_string();
        }
    }
    
    if !current.is_empty() {
        lines.push(Line::from(current));
    }
    
    lines
}
