use chrono::Utc;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs};
use ratatui::Frame;

use crate::action::RepoTab;
use crate::app::App;
use crate::types::{ActionStatus, IssueState, PrState};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    render_tabs(frame, app, chunks[0]);
    render_tab_content(frame, app, chunks[1]);
}

fn render_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let repo_name = app
        .current_repo
        .as_ref()
        .map(|(owner, repo)| format!("{}/{}", owner, repo))
        .unwrap_or_else(|| "Repository".to_string());

    let titles = vec![
        "[P] Pull Requests",
        "[I] Issues",
        "[C] Commits",
        "[A] Actions",
    ];

    let tabs = Tabs::new(titles)
        .block(
            Block::default().borders(Borders::ALL).title(Span::styled(
                format!(" {} ", repo_name),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
        )
        .select(match app.repo_tab {
            RepoTab::PullRequests => 0,
            RepoTab::Issues => 1,
            RepoTab::Commits => 2,
            RepoTab::Actions => 3,
        })
        .style(Style::default().fg(Color::Gray))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(tabs, area);
}

fn render_tab_content(frame: &mut Frame, app: &App, area: Rect) {
    match app.repo_tab {
        RepoTab::PullRequests => render_pr_preview(frame, app, area),
        RepoTab::Issues => render_issues(frame, app, area),
        RepoTab::Commits => render_commits(frame, app, area),
        RepoTab::Actions => render_actions(frame, app, area),
    }
}

fn render_pr_preview(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Pull Requests ({}) ", app.prs.len()));

    if app.prs.is_empty() && !app.loading {
        let empty = Paragraph::new("No open pull requests - Press Enter to view all")
            .block(block)
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(empty, area);
        return;
    }

    let w = area.width.saturating_sub(2) as usize;
    let fixed = 31; // #num(6) + space(1) + state(6) + space(1) + space(1) + @author(16)
    let flex = w.saturating_sub(fixed).max(10);

    let items: Vec<ListItem> = app
        .prs
        .iter()
        .enumerate()
        .map(|(i, pr)| {
            let is_selected = i == app.pr_index;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let state_color = match pr.state {
                PrState::Open => Color::Green,
                PrState::Closed => Color::Red,
                PrState::Merged => Color::Magenta,
            };

            let title = if pr.title.len() > flex {
                format!("{}...", &pr.title[..flex.saturating_sub(3)])
            } else {
                pr.title.clone()
            };

            let author = if pr.author.len() > 15 {
                format!("{}...", &pr.author[..12])
            } else {
                pr.author.clone()
            };

            let line = Line::from(vec![
                Span::styled(
                    format!("#{:<5}", pr.number),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(" "),
                Span::styled(format!("{:6}", pr.state), Style::default().fg(state_color)),
                Span::raw(" "),
                Span::styled(format!("{:<flex$}", title), style),
                Span::raw(" "),
                Span::styled(format!("@{:<15}", author), Style::default().fg(Color::Gray)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::DarkGray));

    let mut state = ListState::default();
    if !app.prs.is_empty() {
        state.select(Some(app.pr_index));
    }

    frame.render_stateful_widget(list, area, &mut state);
}

fn render_issues(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Issues ({}) ", app.issues.len()));

    if app.issues.is_empty() && !app.loading {
        let empty = Paragraph::new("No open issues")
            .block(block)
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(empty, area);
        return;
    }

    let w = area.width.saturating_sub(2) as usize;
    let fixed = 50; // #num(6) + space(1) + state(6) + space(1) + space(1) + labels(18) + space(1) + @author(16)
    let flex = w.saturating_sub(fixed).max(10);

    let items: Vec<ListItem> = app
        .issues
        .iter()
        .enumerate()
        .map(|(i, issue)| {
            let is_selected = i == app.issue_index;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let state_color = match issue.state {
                IssueState::Open => Color::Green,
                IssueState::Closed => Color::Red,
            };

            let title = if issue.title.len() > flex {
                format!("{}...", &issue.title[..flex.saturating_sub(3)])
            } else {
                issue.title.clone()
            };

            let labels = if issue.labels.is_empty() {
                String::new()
            } else {
                let joined = issue.labels.join(", ");
                if joined.len() > 15 {
                    format!("[{}...]", &joined[..12])
                } else {
                    format!("[{}]", joined)
                }
            };

            let author = if issue.author.len() > 15 {
                format!("{}...", &issue.author[..12])
            } else {
                issue.author.clone()
            };

            let line = Line::from(vec![
                Span::styled(
                    format!("#{:<5}", issue.number),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("{:6}", issue.state),
                    Style::default().fg(state_color),
                ),
                Span::raw(" "),
                Span::styled(format!("{:<flex$}", title), style),
                Span::raw(" "),
                Span::styled(
                    format!("{:<18}", labels),
                    Style::default().fg(Color::Magenta),
                ),
                Span::raw(" "),
                Span::styled(format!("@{:<15}", author), Style::default().fg(Color::Gray)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::DarkGray));

    let mut state = ListState::default();
    if !app.issues.is_empty() {
        state.select(Some(app.issue_index));
    }

    frame.render_stateful_widget(list, area, &mut state);
}

fn render_commits(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Commits ({}) ", app.commits.len()));

    if app.commits.is_empty() && !app.loading {
        let empty = Paragraph::new("No commits found")
            .block(block)
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(empty, area);
        return;
    }

    let w = area.width.saturating_sub(2) as usize;
    let fixed = 29; // sha(7) + space(1) + space(1) + @author(16) + space(1) + age(3)
    let flex = w.saturating_sub(fixed).max(10);

    let items: Vec<ListItem> = app
        .commits
        .iter()
        .enumerate()
        .map(|(i, commit)| {
            let is_selected = i == app.commit_index;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let message = if commit.message.len() > flex {
                format!("{}...", &commit.message[..flex.saturating_sub(3)])
            } else {
                commit.message.clone()
            };

            let author = if commit.author.len() > 15 {
                format!("{}...", &commit.author[..12])
            } else {
                commit.author.clone()
            };

            let age = format_age(commit.date);

            let short_sha = &commit.sha[..7.min(commit.sha.len())];

            let line = Line::from(vec![
                Span::styled(short_sha, Style::default().fg(Color::Yellow)),
                Span::raw(" "),
                Span::styled(format!("{:<flex$}", message), style),
                Span::raw(" "),
                Span::styled(format!("@{:<15}", author), Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::styled(format!("{:>3}", age), Style::default().fg(Color::DarkGray)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::DarkGray));

    let mut state = ListState::default();
    if !app.commits.is_empty() {
        state.select(Some(app.commit_index));
    }

    frame.render_stateful_widget(list, area, &mut state);
}

fn render_actions(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Actions ({}) ", app.action_runs.len()));

    if app.action_runs.is_empty() && !app.loading {
        let empty = Paragraph::new("No workflow runs found")
            .block(block)
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(empty, area);
        return;
    }

    let w = area.width.saturating_sub(2) as usize;
    let fixed = 31; // status(2) + space(1) + space(1) + branch(12) + space(1) + event(10) + space(1) + age(3)
    let flex = w.saturating_sub(fixed).max(10);

    let items: Vec<ListItem> = app
        .action_runs
        .iter()
        .enumerate()
        .map(|(i, run)| {
            let is_selected = i == app.action_index;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let (status_icon, status_color) = match run.status {
                ActionStatus::Completed => {
                    if let Some(conclusion) = &run.conclusion {
                        (
                            conclusion.to_string(),
                            match conclusion {
                                crate::types::ActionConclusion::Success => Color::Green,
                                crate::types::ActionConclusion::Failure => Color::Red,
                                _ => Color::Yellow,
                            },
                        )
                    } else {
                        ("?".to_string(), Color::Gray)
                    }
                }
                ActionStatus::InProgress => ("⟳".to_string(), Color::Yellow),
                ActionStatus::Queued => ("◯".to_string(), Color::Gray),
            };

            let name = if run.name.len() > flex {
                format!("{}...", &run.name[..flex.saturating_sub(3)])
            } else {
                run.name.clone()
            };

            let branch = if run.branch.len() > 12 {
                format!("{}...", &run.branch[..9])
            } else {
                run.branch.clone()
            };

            let age = format_age(run.created_at);

            let line = Line::from(vec![
                Span::styled(
                    format!("{:<2}", status_icon),
                    Style::default().fg(status_color),
                ),
                Span::raw(" "),
                Span::styled(format!("{:<flex$}", name), style),
                Span::raw(" "),
                Span::styled(format!("{:<12}", branch), Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::styled(
                    format!("{:<10}", run.event),
                    Style::default().fg(Color::Gray),
                ),
                Span::raw(" "),
                Span::styled(format!("{:>3}", age), Style::default().fg(Color::DarkGray)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::DarkGray));

    let mut state = ListState::default();
    if !app.action_runs.is_empty() {
        state.select(Some(app.action_index));
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
