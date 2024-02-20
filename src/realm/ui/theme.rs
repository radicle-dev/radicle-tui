use tuirealm::props::BorderType;

#[derive(Debug, Clone)]
pub struct Icons {
    pub property_divider: char,
    pub shortcutbar_divider: char,
    pub tab_divider: char,
    pub tab_overline: char,
    pub whitespace: char,
}

#[derive(Debug, Clone)]
pub struct Tables {
    pub spacing: u16,
}

/// The Radicle TUI theme. In the future, it might be defined in a JSON
/// config file.
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub icons: Icons,
    pub tables: Tables,
    pub border_type: BorderType,
}

impl Default for Theme {
    fn default() -> Theme {
        Theme {
            name: String::from("Default"),
            icons: Icons {
                property_divider: '∙',
                shortcutbar_divider: '∙',
                tab_divider: '|',
                tab_overline: '▔',
                whitespace: ' ',
            },
            tables: Tables { spacing: 2 },
            border_type: BorderType::Rounded,
        }
    }
}

pub mod style {
    use tuirealm::props::{Color, Style, TextModifiers};

    pub fn reset() -> Style {
        Style::default().fg(Color::Reset)
    }

    pub fn reset_dim() -> Style {
        Style::default()
            .fg(Color::Reset)
            .add_modifier(TextModifiers::DIM)
    }

    pub fn red() -> Style {
        Style::default().fg(Color::Red)
    }

    pub fn green() -> Style {
        Style::default().fg(Color::Green)
    }

    pub fn yellow() -> Style {
        Style::default().fg(Color::Yellow)
    }

    pub fn yellow_dim() -> Style {
        yellow().add_modifier(TextModifiers::DIM)
    }

    pub fn yellow_dim_reversed() -> Style {
        yellow_dim().add_modifier(TextModifiers::REVERSED)
    }

    pub fn blue() -> Style {
        Style::default().fg(Color::Blue)
    }

    pub fn magenta() -> Style {
        Style::default().fg(Color::Magenta)
    }

    pub fn magenta_dim() -> Style {
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(TextModifiers::DIM)
    }

    pub fn cyan() -> Style {
        Style::default().fg(Color::Cyan)
    }

    pub fn lightblue() -> Style {
        Style::default().fg(Color::LightBlue)
    }

    pub fn gray() -> Style {
        Style::default().fg(Color::Gray)
    }

    pub fn gray_dim() -> Style {
        Style::default()
            .fg(Color::Gray)
            .add_modifier(TextModifiers::DIM)
    }

    pub fn darkgray() -> Style {
        Style::default().fg(Color::DarkGray)
    }

    pub fn reversed() -> Style {
        Style::default().add_modifier(TextModifiers::REVERSED)
    }

    pub fn default_reversed() -> Style {
        Style::default()
            .fg(Color::DarkGray)
            // .add_modifier(TextModifiers::DIM)
            .add_modifier(TextModifiers::REVERSED)
    }

    pub fn magenta_reversed() -> Style {
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(TextModifiers::REVERSED)
    }

    pub fn yellow_reversed() -> Style {
        Style::default().fg(Color::DarkGray).bg(Color::Yellow)
    }

    pub fn border(focus: bool) -> Style {
        if focus {
            gray_dim()
        } else {
            darkgray()
        }
    }

    pub fn highlight() -> Style {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(TextModifiers::REVERSED)
    }
}
