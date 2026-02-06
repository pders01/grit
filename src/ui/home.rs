use chrono::Utc;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::{App, HomeSection};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    // Split the area into two sections: review requests and my PRs
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_review_requests(frame, app, chunks[0]);
    render_my_prs(frame, app, chunks[1]);
}

fn render_review_requests(frame: &mut Frame, app: &App, area: Rect) {
    let is_active = app.home_section == HomeSection::ReviewRequests;

    let title_style = if is_active {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            format!(" Review Requests ({}) ", app.review_requests.len()),
            title_style,
        ))
        .border_style(if is_active {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        });

    if app.review_requests.is_empty() && !app.loading {
        let empty = Paragraph::new("No review requests")
            .block(block)
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(empty, area);
        return;
    }

    let w = area.width.saturating_sub(2) as usize;
    let fixed = 57; // repo(25) + space(1) + #num(6) + space(1) + spaces(2) + @author(~16) + spaces(2) + age(~4)
    let flex = w.saturating_sub(fixed).max(10);

    let items: Vec<ListItem> = app
        .review_requests
        .iter()
        .enumerate()
        .map(|(i, req)| {
            let is_selected = is_active && i == app.review_index;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let repo = format!("{}/{}", req.repo_owner, req.repo_name);
            let repo_display = if repo.len() > 25 {
                format!("{}...", &repo[..22])
            } else {
                repo
            };

            let title = if req.pr_title.len() > flex {
                format!("{}...", &req.pr_title[..flex.saturating_sub(3)])
            } else {
                req.pr_title.clone()
            };

            let age = format_age(req.updated_at);

            let line = Line::from(vec![
                Span::styled(
                    format!("{:<25}", repo_display),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("#{:<5}", req.pr_number),
                    Style::default().fg(Color::Gray),
                ),
                Span::raw(" "),
                Span::styled(format!("{:<flex$}", title), style),
                Span::raw("  "),
                Span::styled(format!("@{}", req.author), Style::default().fg(Color::Gray)),
                Span::raw("  "),
                Span::styled(age, Style::default().fg(Color::DarkGray)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::DarkGray));

    let mut state = ListState::default();
    if is_active && !app.review_requests.is_empty() {
        state.select(Some(app.review_index));
    }

    frame.render_stateful_widget(list, area, &mut state);
}

fn render_my_prs(frame: &mut Frame, app: &App, area: Rect) {
    let is_active = app.home_section == HomeSection::MyPrs;

    let title_style = if is_active {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            format!(" Your Open PRs ({}) ", app.my_prs.len()),
            title_style,
        ))
        .border_style(if is_active {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        });

    if app.my_prs.is_empty() && !app.loading {
        let empty = Paragraph::new("No open pull requests")
            .block(block)
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(empty, area);
        return;
    }

    let w = area.width.saturating_sub(2) as usize;
    let fixed = 43; // repo(25) + space(1) + #num(6) + space(1) + spaces(2) + status(~8)
    let flex = w.saturating_sub(fixed).max(10);

    let items: Vec<ListItem> = app
        .my_prs
        .iter()
        .enumerate()
        .map(|(i, pr)| {
            let is_selected = is_active && i == app.my_pr_index;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let repo = format!("{}/{}", pr.repo_owner, pr.repo_name);
            let repo_display = if repo.len() > 25 {
                format!("{}...", &repo[..22])
            } else {
                repo
            };

            let title = if pr.title.len() > flex {
                format!("{}...", &pr.title[..flex.saturating_sub(3)])
            } else {
                pr.title.clone()
            };

            let status = pr.checks_status.to_string();
            let status_color = match pr.checks_status {
                crate::types::ChecksStatus::Success => Color::Green,
                crate::types::ChecksStatus::Failure => Color::Red,
                crate::types::ChecksStatus::Pending => Color::Yellow,
                crate::types::ChecksStatus::None => Color::Gray,
            };

            let line = Line::from(vec![
                Span::styled(
                    format!("{:<25}", repo_display),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("#{:<5}", pr.number),
                    Style::default().fg(Color::Gray),
                ),
                Span::raw(" "),
                Span::styled(format!("{:<flex$}", title), style),
                Span::raw("  "),
                Span::styled(status, Style::default().fg(status_color)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::DarkGray));

    let mut state = ListState::default();
    if is_active && !app.my_prs.is_empty() {
        state.select(Some(app.my_pr_index));
    }

    frame.render_stateful_widget(list, area, &mut state);
}

fn format_age(dt: chrono::DateTime<chrono::Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(dt);

    if duration.num_days() > 0 {
        format!("{}d", duration.num_days())
    } else if duration.num_hours() > 0 {
        format!("{}h", duration.num_hours())
    } else if duration.num_minutes() > 0 {
        format!("{}m", duration.num_minutes())
    } else {
        "now".to_string()
    }
}
