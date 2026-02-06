use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    if app.repos.is_empty() && !app.loading {
        let block = Block::default().borders(Borders::ALL).title("Repositories");
        let empty = ratatui::widgets::Paragraph::new("No repositories found")
            .block(block)
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(empty, area);
        return;
    }

    let w = area.width.saturating_sub(2) as usize;
    let fixed = 40; // repo_name(30) + space(1) + stars(7) + spaces(2)
    let flex = w.saturating_sub(fixed).max(10);

    let items: Vec<ListItem> = app
        .repos
        .iter()
        .enumerate()
        .map(|(i, repo)| {
            let style = if i == app.repo_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let description = repo
                .description
                .as_ref()
                .map(|d| {
                    if d.len() > flex {
                        format!("{}...", &d[..flex.saturating_sub(3)])
                    } else {
                        d.clone()
                    }
                })
                .unwrap_or_default();

            let repo_name = format!("{}/{}", repo.owner, repo.name);
            let repo_display = if repo_name.len() > 30 {
                format!("{}...", &repo_name[..27])
            } else {
                repo_name
            };

            let line = Line::from(vec![
                Span::styled(format!("{:<30}", repo_display), style),
                Span::raw(" "),
                Span::styled(
                    format!("★ {:>5}", repo.stars),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("{:<flex$}", description),
                    Style::default().fg(Color::Gray),
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Repositories ({})", app.repos.len())),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));

    let mut state = ListState::default();
    state.select(Some(app.repo_index));

    frame.render_stateful_widget(list, area, &mut state);
}
