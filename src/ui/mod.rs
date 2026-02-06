mod commit_detail;
mod home;
mod popup;
mod pr_detail;
mod repo_list;
mod repo_view;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, InputMode, Screen, SearchState};

use crate::action::ConfirmAction;

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

    // Render popup overlays
    match &app.input_mode {
        InputMode::Confirm => {
            if let Some(confirm) = &app.confirm_action {
                let (title, message) = match confirm {
                    ConfirmAction::ClosePr(n) => {
                        ("Close PR".to_string(), format!("Close PR #{}?", n))
                    }
                    ConfirmAction::MergePr { number, method } => (
                        "Merge PR".to_string(),
                        format!("Merge PR #{} via {}?", number, method),
                    ),
                    ConfirmAction::CloseIssue(n) => {
                        ("Close Issue".to_string(), format!("Close issue #{}?", n))
                    }
                };
                popup::render_confirm(frame, &title, &message);
            }
        }
        InputMode::SelectPopup => {
            popup::render_select(frame, &app.popup_title, &app.popup_items, app.popup_index);
        }
        _ => {}
    }
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
    // Search input mode takes over status bar
    if app.input_mode == InputMode::Search {
        let line = Line::from(vec![
            Span::styled("/", Style::default().fg(Color::Yellow)),
            Span::styled(&app.search.query, Style::default().fg(Color::White)),
            Span::styled("_", Style::default().fg(Color::Yellow)),
        ]);
        let bar = Paragraph::new(line).style(Style::default().bg(Color::DarkGray));
        frame.render_widget(bar, area);
        return;
    }

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
    } else if let Some((msg, instant)) = &app.flash_message {
        if instant.elapsed() < std::time::Duration::from_secs(3) {
            Line::from(vec![Span::styled(
                msg.clone(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )])
        } else {
            Line::from(vec![Span::styled("", Style::default())])
        }
    } else if app.search.active {
        let total = if !app.search.match_indices.is_empty() {
            app.search.match_indices.len()
        } else {
            app.search.content_matches.len()
        };
        let current = if total > 0 {
            app.search.current_match + 1
        } else {
            0
        };
        Line::from(vec![
            Span::styled(
                format!("[{}/{}]", current, total),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(" "),
            Span::styled(
                format!("\"{}\"", app.search.query),
                Style::default().fg(Color::White),
            ),
            Span::raw("  "),
            Span::styled(
                "n/N: next/prev | Esc: clear",
                Style::default().fg(Color::Gray),
            ),
        ])
    } else {
        let help = match app.screen {
            Screen::Home => "/ search | r repos | f forge | o open | y yank | Enter open | q quit",
            Screen::RepoList => "/ search | r refresh | o open | y yank | Enter select | q back",
            Screen::RepoView => match app.repo_tab {
                crate::action::RepoTab::Issues => {
                    "/ search | x close | C comment | o open | y yank | q back"
                }
                _ => "/ search | r refresh | o open | y yank | Enter detail | q back",
            },
            Screen::PrDetail => {
                "d diff | m merge | x close | C comment | R review | o open | q back"
            }
            Screen::CommitDetail => "d diff | / search | o open | y yank | q back",
        };
        Line::from(vec![
            Span::styled(
                format!("[{}] ", app.forge_name),
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::DIM),
            ),
            Span::styled(help, Style::default().fg(Color::Gray)),
        ])
    };

    let status_bar = Paragraph::new(status).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(status_bar, area);
}

/// Highlight search matches within a line of text.
/// Returns an owned `Line<'static>` so callers don't have lifetime issues.
pub fn highlight_line(
    text: &str,
    line_idx: usize,
    base_style: Style,
    search: &SearchState,
) -> Line<'static> {
    if !search.active || search.query.is_empty() || search.content_matches.is_empty() {
        return Line::from(Span::styled(text.to_string(), base_style));
    }

    // Collect matches for this line
    let line_matches: Vec<(usize, usize, bool)> = search
        .content_matches
        .iter()
        .enumerate()
        .filter(|(_, (li, _, _))| *li == line_idx)
        .map(|(match_global_idx, (_, start, end))| {
            let is_current = match_global_idx == search.current_match;
            (*start, *end, is_current)
        })
        .collect();

    if line_matches.is_empty() {
        return Line::from(Span::styled(text.to_string(), base_style));
    }

    let mut spans = Vec::new();
    let mut pos = 0;
    for (start, end, is_current) in &line_matches {
        let start = (*start).min(text.len());
        let end = (*end).min(text.len());
        if pos < start {
            spans.push(Span::styled(text[pos..start].to_string(), base_style));
        }
        let highlight_style = if *is_current {
            Style::default().bg(Color::Red).fg(Color::White)
        } else {
            Style::default().bg(Color::Yellow).fg(Color::Black)
        };
        spans.push(Span::styled(text[start..end].to_string(), highlight_style));
        pos = end;
    }
    if pos < text.len() {
        spans.push(Span::styled(text[pos..].to_string(), base_style));
    }

    Line::from(spans)
}
