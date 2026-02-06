use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::{
    event::{self, Event as CrosstermEvent, EventStream},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tokio::time::interval;
use tokio_util::sync::CancellationToken;

use crate::event::Event;

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

pub fn init() -> io::Result<Tui> {
    execute!(io::stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    Terminal::new(CrosstermBackend::new(io::stdout()))
}

pub fn restore() -> io::Result<()> {
    execute!(io::stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()
}

/// Drain any pending crossterm events (e.g. leftover keystrokes after a pager exit).
/// Must be called while raw mode is enabled.
pub fn drain_events() {
    while event::poll(Duration::ZERO).unwrap_or(false) {
        let _ = event::read();
    }
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<Event>,
    cancel: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration, render_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let cancel = CancellationToken::new();
        let task_cancel = cancel.clone();

        let task = tokio::spawn(async move {
            let mut reader = EventStream::new();
            let mut tick_interval = interval(tick_rate);
            let mut render_interval = interval(render_rate);

            loop {
                tokio::select! {
                    _ = task_cancel.cancelled() => break,
                    _ = tick_interval.tick() => {
                        tx.send(Event::Tick).ok();
                    }
                    _ = render_interval.tick() => {
                        tx.send(Event::Render).ok();
                    }
                    Some(Ok(evt)) = reader.next() => {
                        if let CrosstermEvent::Key(key) = evt {
                            if key.kind == event::KeyEventKind::Press {
                                tx.send(Event::Key(key)).ok();
                            }
                        }
                    }
                }
            }
        });

        Self { rx, cancel, task }
    }

    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}

impl Drop for EventHandler {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.task.abort();
    }
}
