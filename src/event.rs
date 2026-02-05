use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone)]
pub enum Event {
    Init,
    Tick,
    Render,
    Key(KeyEvent),
}

impl Event {
    pub fn is_quit(&self) -> bool {
        matches!(
            self,
            Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            })
        )
    }
}
