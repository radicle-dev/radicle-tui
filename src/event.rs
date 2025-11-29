use ratatui::crossterm::{self, event::KeyModifiers};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Key {
    Char(char),
    Alt(char),
    Ctrl(char),
    Enter,
    Backspace,
    Tab,
    BackTab,
    Delete,
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
    Esc,
    Unknown,
}

#[derive(Clone, Copy, Debug)]
pub enum Event {
    Key(Key),
    Resize(u16, u16),
    Unknown,
}

impl From<crossterm::event::Event> for Event {
    fn from(event: crossterm::event::Event) -> Self {
        use crossterm::event::KeyCode;

        match event {
            crossterm::event::Event::Key(key) => match (key.code, key.modifiers) {
                (KeyCode::Char(c), KeyModifiers::CONTROL) => Event::Key(Key::Ctrl(c)),
                (KeyCode::Char(c), KeyModifiers::ALT) => Event::Key(Key::Alt(c)),
                (KeyCode::Char(c), _) => Event::Key(Key::Char(c)),
                (KeyCode::Enter, _) => Event::Key(Key::Enter),
                (KeyCode::Backspace, _) => Event::Key(Key::Backspace),
                (KeyCode::Tab, _) => Event::Key(Key::Tab),
                (KeyCode::BackTab, _) => Event::Key(Key::BackTab),
                (KeyCode::Delete, _) => Event::Key(Key::Delete),
                (KeyCode::Up, _) => Event::Key(Key::Up),
                (KeyCode::Down, _) => Event::Key(Key::Down),
                (KeyCode::Left, _) => Event::Key(Key::Left),
                (KeyCode::Right, _) => Event::Key(Key::Right),
                (KeyCode::PageUp, _) => Event::Key(Key::PageUp),
                (KeyCode::PageDown, _) => Event::Key(Key::PageDown),
                (KeyCode::Home, _) => Event::Key(Key::Home),
                (KeyCode::Esc, _) => Event::Key(Key::Esc),
                (KeyCode::End, _) => Event::Key(Key::End),
                _ => Event::Key(Key::Unknown),
            },
            crossterm::event::Event::Resize(x, y) => Event::Resize(x, y),
            _ => Event::Unknown,
        }
    }
}
