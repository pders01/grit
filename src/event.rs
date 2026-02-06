use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone)]
pub enum Event {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventKind;

    fn key_event(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        })
    }

    #[test]
    fn is_quit_ctrl_c() {
        assert!(key_event(KeyCode::Char('c'), KeyModifiers::CONTROL).is_quit());
    }

    #[test]
    fn is_quit_plain_c() {
        assert!(!key_event(KeyCode::Char('c'), KeyModifiers::NONE).is_quit());
    }

    #[test]
    fn is_quit_tick() {
        assert!(!Event::Tick.is_quit());
    }

    #[test]
    fn is_quit_render() {
        assert!(!Event::Render.is_quit());
    }
}
