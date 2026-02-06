use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::types::PrState;

use super::highlight_line;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let Some(pr) = &app.current_pr else {
        let block = Block::default().borders(Borders::ALL).title("Pull Request");
        let empty = Paragraph::new("No pull request selected")
            .block(block)
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(empty, area);
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(0)])
        .split(area);

    // Header section with PR metadata
    render_header(frame, pr, chunks[0]);

    // Body section with description
    render_body(frame, app, pr, chunks[1]);
}

fn render_header(frame: &mut Frame, pr: &crate::types::PullRequest, area: Rect) {
    let state_color = match pr.state {
        PrState::Open => Color::Green,
        PrState::Closed => Color::Red,
        PrState::Merged => Color::Magenta,
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(
                format!("#{} ", pr.number),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(&pr.title, Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled(
                format!("{}", pr.state),
                Style::default()
                    .fg(state_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" | "),
            Span::styled(
                format!("@{}", pr.author),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(" wants to merge "),
            Span::styled(&pr.head_branch, Style::default().fg(Color::Cyan)),
            Span::raw(" into "),
            Span::styled(&pr.base_branch, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled(
                format!("+{}", pr.stats.additions),
                Style::default().fg(Color::Green),
            ),
            Span::raw(" "),
            Span::styled(
                format!("-{}", pr.stats.deletions),
                Style::default().fg(Color::Red),
            ),
            Span::raw(" | "),
            Span::raw(format!("{} files changed", pr.stats.changed_files)),
            Span::raw(" | "),
            Span::raw(format!("{} commits", pr.stats.commits)),
            Span::raw(" | "),
            Span::raw(format!("{} comments", pr.stats.comments)),
        ]),
        Line::from(vec![
            Span::styled("Created: ", Style::default().fg(Color::Gray)),
            Span::raw(pr.created_at.format("%Y-%m-%d %H:%M").to_string()),
            Span::raw(" | "),
            Span::styled("Updated: ", Style::default().fg(Color::Gray)),
            Span::raw(pr.updated_at.format("%Y-%m-%d %H:%M").to_string()),
        ]),
    ];

    let header =
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Details"));

    frame.render_widget(header, area);
}

fn render_body(frame: &mut Frame, app: &App, pr: &crate::types::PullRequest, area: Rect) {
    let body_text = pr.body.as_deref().unwrap_or("No description provided.");

    // Build lines with search highlighting
    let lines: Vec<Line> = body_text
        .lines()
        .enumerate()
        .map(|(line_idx, l)| {
            let text = l.replace('\t', "    ");
            highlight_line(&text, line_idx, Style::default(), &app.search)
        })
        .collect();

    // Calculate visible area (account for borders)
    let inner_height = area.height.saturating_sub(2) as usize;

    // Clamp scroll offset to content bounds
    let max_scroll = lines.len().saturating_sub(inner_height);
    let scroll_offset = app.scroll_offset.min(max_scroll);

    // Slice lines to visible range
    let visible_lines: Vec<Line> = lines
        .into_iter()
        .skip(scroll_offset)
        .take(inner_height)
        .collect();

    // Clear the area first to prevent artifacts
    frame.render_widget(Clear, area);

    let body = Paragraph::new(Text::from(visible_lines))
        .block(Block::default().borders(Borders::ALL).title("Description"));

    frame.render_widget(body, area);
}
