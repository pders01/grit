use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

/// Render a centered confirmation popup: [y]es / [n]o
pub fn render_confirm(frame: &mut Frame, title: &str, message: &str) {
    let area = centered_rect(50, 7, frame.area());
    frame.render_widget(Clear, area);

    let lines = vec![
        Line::from(""),
        Line::from(Span::raw(message)),
        Line::from(""),
        Line::from(vec![
            Span::styled("[y]", Style::default().fg(Color::Green)),
            Span::raw("es  "),
            Span::styled("[n]", Style::default().fg(Color::Red)),
            Span::raw("o"),
        ]),
    ];

    let popup = Paragraph::new(lines)
        .block(
            Block::default().borders(Borders::ALL).title(Span::styled(
                format!(" {} ", title),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
        )
        .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(popup, area);
}

/// Render a centered selectable list popup
pub fn render_select(frame: &mut Frame, title: &str, items: &[String], selected: usize) {
    let height = (items.len() + 2).min(12) as u16; // +2 for borders
    let area = centered_rect(40, height, frame.area());
    frame.render_widget(Clear, area);

    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let style = if i == selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let prefix = if i == selected { "> " } else { "  " };
            ListItem::new(Line::from(Span::styled(
                format!("{}{}", prefix, item),
                style,
            )))
        })
        .collect();

    let list = List::new(list_items).block(
        Block::default().borders(Borders::ALL).title(Span::styled(
            format!(" {} ", title),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
    );

    let mut state = ListState::default();
    state.select(Some(selected));
    frame.render_stateful_widget(list, area, &mut state);
}

/// Create a centered rect using percentage of the outer rect
fn centered_rect(width: u16, height: u16, outer: Rect) -> Rect {
    let popup_width = width.min(outer.width);
    let popup_height = height.min(outer.height);

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((outer.height.saturating_sub(popup_height)) / 2),
            Constraint::Length(popup_height),
            Constraint::Min(0),
        ])
        .split(outer);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length((outer.width.saturating_sub(popup_width)) / 2),
            Constraint::Length(popup_width),
            Constraint::Min(0),
        ])
        .split(vertical[1]);

    horizontal[1]
}
