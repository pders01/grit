mod action;
mod app;
mod error;
mod event;
mod github;
mod tui;
mod types;
mod ui;

use std::panic;
use std::time::Duration;

use tokio::sync::mpsc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::action::Action;
use crate::app::App;
use crate::error::GritError;
use crate::event::Event;
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

    // Get GitHub token from environment
    let token = std::env::var("GITHUB_TOKEN")
        .map_err(|_| GritError::Auth("GITHUB_TOKEN environment variable not set".to_string()))?;

    // Initialize GitHub client
    let github = GitHub::new(token)?;

    // Run the application
    let result = run(github).await;

    // Restore terminal
    tui::restore()?;

    result
}

async fn run(github: GitHub) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize terminal
    let mut terminal = tui::init()?;

    // Create action channel
    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();

    // Create app state
    let mut app = App::new(github, action_tx.clone());

    // Create event handler
    let tick_rate = Duration::from_millis(250);
    let render_rate = Duration::from_millis(16); // ~60fps
    let mut events = EventHandler::new(tick_rate, render_rate);

    // Main loop
    loop {
        // Handle events and actions
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
                app.update(action);
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
