use chrono::Utc;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let Some(commit) = &app.current_commit else {
        let block = Block::default().borders(Borders::ALL).title(" Commit ");
        let empty = Paragraph::new("No commit loaded")
            .block(block)
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(empty, area);
        return;
    };

    let short_sha = &commit.sha[..7.min(commit.sha.len())];
    let age = format_age(commit.date);

    let mut lines: Vec<Line> = vec![
        // Header
        Line::from(vec![
            Span::styled(
                format!("Commit {}", short_sha),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("@{}", commit.author),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw("  "),
            Span::styled(age, Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        // Stats
        Line::from(vec![
            Span::styled(
                format!("+{}", commit.stats.additions),
                Style::default().fg(Color::Green),
            ),
            Span::raw("  "),
            Span::styled(
                format!("-{}", commit.stats.deletions),
                Style::default().fg(Color::Red),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{} files changed", commit.files.len()),
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(""),
        // Message
        Line::from(Span::styled(
            "Message:",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];

    // Add commit message lines
    for msg_line in commit.message.lines() {
        lines.push(Line::from(Span::styled(
            format!("  {}", msg_line),
            Style::default().fg(Color::White),
        )));
    }

    lines.push(Line::from(""));

    // Add file diffs
    for file in &commit.files {
        let status_color = match file.status.as_str() {
            "added" => Color::Green,
            "removed" => Color::Red,
            "modified" => Color::Yellow,
            "renamed" => Color::Cyan,
            _ => Color::Gray,
        };

        let status_char = match file.status.as_str() {
            "added" => "A",
            "removed" => "D",
            "modified" => "M",
            "renamed" => "R",
            _ => "?",
        };

        // File header
        lines.push(Line::from(vec![
            Span::styled(
                format!("─── {} ", status_char),
                Style::default().fg(status_color),
            ),
            Span::styled(
                &file.filename,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("+{}", file.additions),
                Style::default().fg(Color::Green),
            ),
            Span::raw(" "),
            Span::styled(
                format!("-{}", file.deletions),
                Style::default().fg(Color::Red),
            ),
            Span::styled(" ───", Style::default().fg(Color::DarkGray)),
        ]));

        // Show diff if available
        if let Some(patch) = &file.patch {
            for diff_line in patch.lines() {
                // Replace tabs with spaces to avoid rendering issues
                let sanitized = diff_line.replace('\t', "    ");

                let color = if sanitized.starts_with('+') && !sanitized.starts_with("+++") {
                    Color::Green
                } else if sanitized.starts_with('-') && !sanitized.starts_with("---") {
                    Color::Red
                } else if sanitized.starts_with("@@") {
                    Color::Cyan
                } else {
                    Color::Gray
                };

                lines.push(Line::from(Span::styled(
                    sanitized,
                    Style::default().fg(color),
                )));
            }
        }

        lines.push(Line::from(""));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Commit {} ", short_sha));

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

    let paragraph = Paragraph::new(visible_lines).block(block);
    frame.render_widget(paragraph, area);
}

fn format_age(dt: chrono::DateTime<chrono::Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(dt);

    if duration.num_days() > 0 {
        format!("{}d ago", duration.num_days())
    } else if duration.num_hours() > 0 {
        format!("{}h ago", duration.num_hours())
    } else if duration.num_minutes() > 0 {
        format!("{}m ago", duration.num_minutes())
    } else {
        "just now".to_string()
    }
}
