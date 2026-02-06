mod action;
mod app;
mod auth;
mod cache;
mod config;
mod error;
mod event;
mod forge;
mod gitea;
mod github;
mod gitlab;
mod pager;
mod tui;
mod types;
mod ui;

use std::panic;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::action::{Action, EditorContext};
use crate::app::App;
use crate::config::{Config, ForgeType};
use crate::event::Event;
use crate::forge::Forge;
use crate::github::GitHub;
use crate::tui::EventHandler;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    // Set up panic hook to restore terminal
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = tui::restore();
        original_hook(panic_info);
    }));

    // Load config and detect forge
    let config = Config::load();
    let forge_config = config::detect_forge(&config)
        .or_else(|| config.forges.first())
        .ok_or("No forge configured")?
        .clone();

    // Load token for the detected forge
    let token = auth::load_forge_token(&forge_config)
        .await
        .map_err(Box::<dyn std::error::Error>::from)?;

    // Initialize forge client
    let forge: Arc<dyn Forge> = match forge_config.forge_type {
        ForgeType::GitHub => Arc::new(GitHub::new(token)?),
        ForgeType::GitLab => Arc::new(gitlab::GitLab::new(forge_config.host.clone(), token)),
        ForgeType::Gitea => Arc::new(gitea::Gitea::new(forge_config.host.clone(), token)),
    };

    // Run the application
    let result = run(forge).await;

    // Restore terminal
    tui::restore()?;

    result
}

/// Actions that require suspending the TUI and shelling out
enum SuspendAction {
    Pager(String),
    Editor(EditorContext),
}

async fn run(forge: Arc<dyn Forge>) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize terminal
    let mut terminal = tui::init()?;

    // Create action channel
    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();

    // Create app state
    let mut app = App::new(forge, action_tx.clone());

    // Create event handler
    let tick_rate = Duration::from_millis(250);
    let render_rate = Duration::from_millis(16); // ~60fps
    let mut events = EventHandler::new(tick_rate, render_rate);

    // Trigger initial data load (not from EventHandler to avoid re-triggering after pager suspend)
    action_tx.send(Action::LoadHome)?;

    // Main loop
    loop {
        // Collect any suspend action to handle AFTER the select block,
        // so we can drop the event handler before shelling out.
        let mut suspend: Option<SuspendAction> = None;

        tokio::select! {
            Some(event) = events.next() => {
                if event.is_quit() {
                    break;
                }

                match event {
                    Event::Render => {
                        terminal.draw(|frame| ui::render(frame, &app))?;
                    }
                    _ => {
                        let action = app.handle_event(event);
                        if !matches!(action, Action::None) {
                            action_tx.send(action)?;
                        }
                    }
                }
            }
            Some(action) = action_rx.recv() => {
                match action {
                    Action::SuspendForPager(content) => {
                        suspend = Some(SuspendAction::Pager(content));
                    }
                    Action::SuspendForEditor(ctx) => {
                        suspend = Some(SuspendAction::Editor(ctx));
                    }
                    other => {
                        app.update(other);
                    }
                }
            }
        }

        // Handle suspend actions outside the select block.
        // Drop the event handler first so its background task stops
        // polling crossterm — otherwise it steals keystrokes from the
        // child pager/editor process.
        if let Some(action) = suspend {
            drop(events);
            tui::restore()?;

            match action {
                SuspendAction::Pager(content) => {
                    let pager_cmd = pager::detect_pager();
                    let _ = pager::open_pager(&content, &pager_cmd);
                }
                SuspendAction::Editor(ctx) => {
                    if let Some(body) = open_editor() {
                        if !body.trim().is_empty() {
                            match ctx {
                                EditorContext::CommentOnPr {
                                    owner,
                                    repo,
                                    number,
                                }
                                | EditorContext::CommentOnIssue {
                                    owner,
                                    repo,
                                    number,
                                } => {
                                    app.spawn_comment(owner, repo, number, body);
                                }
                                EditorContext::ReviewPr {
                                    owner,
                                    repo,
                                    number,
                                    event,
                                } => {
                                    app.spawn_submit_review(owner, repo, number, event, body);
                                }
                            }
                        }
                    }
                }
            }

            terminal = tui::init()?;
            // Discard leftover keystrokes (e.g. extra q's from exiting the pager)
            tui::drain_events();
            events = EventHandler::new(tick_rate, render_rate);
            continue;
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

/// Open $EDITOR with a temp file, return contents if saved
fn open_editor() -> Option<String> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join(format!("grit-{}.md", std::process::id()));

    // Write empty file
    std::fs::write(&tmp_path, "").ok()?;

    let status = std::process::Command::new("sh")
        .args(["-c", &format!("{} {}", editor, tmp_path.display())])
        .status()
        .ok()?;

    if !status.success() {
        let _ = std::fs::remove_file(&tmp_path);
        return None;
    }

    let content = std::fs::read_to_string(&tmp_path).ok()?;
    let _ = std::fs::remove_file(&tmp_path);
    Some(content)
}
