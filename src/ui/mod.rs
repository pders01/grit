mod commit_detail;
mod home;
mod pr_detail;
mod repo_list;
mod repo_view;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, Screen};

pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_header(frame, app, chunks[0]);

    match app.screen {
        Screen::Home => home::render(frame, app, chunks[1]),
        Screen::RepoList => repo_list::render(frame, app, chunks[1]),
        Screen::RepoView => repo_view::render(frame, app, chunks[1]),
        Screen::PrDetail => pr_detail::render(frame, app, chunks[1]),
        Screen::CommitDetail => commit_detail::render(frame, app, chunks[1]),
    }

    render_status_bar(frame, app, chunks[2]);
}

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let title = match app.screen {
        Screen::Home => "grit - Home".to_string(),
        Screen::RepoList => "grit - Repositories".to_string(),
        Screen::RepoView => {
            if let Some((owner, repo)) = &app.current_repo {
                format!("grit - {}/{}", owner, repo)
            } else {
                "grit - Repository".to_string()
            }
        }
        Screen::PrDetail => {
            if let Some(pr) = &app.current_pr {
                format!("grit - PR #{}: {}", pr.number, pr.title)
            } else {
                "grit - Pull Request".to_string()
            }
        }
        Screen::CommitDetail => {
            if let Some(commit) = &app.current_commit {
                let short_sha = &commit.sha[..7.min(commit.sha.len())];
                let msg_line = commit.message.lines().next().unwrap_or("");
                let msg = if msg_line.len() > 50 {
                    format!("{}...", &msg_line[..47])
                } else {
                    msg_line.to_string()
                };
                format!("grit - Commit {}: {}", short_sha, msg)
            } else {
                "grit - Commit".to_string()
            }
        }
    };

    let header = Paragraph::new(Line::from(vec![Span::styled(
        title,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )]))
    .style(Style::default().bg(Color::DarkGray));

    frame.render_widget(header, area);
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let status = if let Some(error) = &app.error {
        Line::from(vec![Span::styled(
            format!("Error: {}", error),
            Style::default().fg(Color::Red),
        )])
    } else if app.loading {
        Line::from(vec![Span::styled(
            "Loading...",
            Style::default().fg(Color::Yellow),
        )])
    } else {
        let help = match app.screen {
            Screen::Home => {
                "h/l: sections | j/k/g/G: nav | Ctrl+d/u: page | Enter: open | r: repos | q: quit"
            }
            Screen::RepoList => "j/k/g/G: nav | Ctrl+d/u: page | Enter: select | q: back",
            Screen::RepoView => "h/l: tabs | j/k/g/G: nav | Ctrl+d/u: page | Enter: open | q: back",
            Screen::PrDetail | Screen::CommitDetail => "j/k/g/G: scroll | Ctrl+d/u: page | q: back",
        };
        Line::from(vec![Span::styled(help, Style::default().fg(Color::Gray))])
    };

    let status_bar = Paragraph::new(status).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(status_bar, area);
}
