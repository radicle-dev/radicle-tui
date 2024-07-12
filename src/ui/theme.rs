use ratatui::style::Color;

#[derive(Clone, Default)]
pub enum Mode {
    #[default]
    Light,
    Dark,
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub border_color: Color,
    pub focus_border_color: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            border_color: Color::Indexed(236),
            focus_border_color: Color::Indexed(238),
        }
    }
}

pub mod style {
    use ratatui::style::{Color, Style, Stylize};

    pub fn reset() -> Style {
        Style::default().fg(Color::Reset)
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

    pub fn blue() -> Style {
        Style::default().fg(Color::Blue)
    }

    pub fn magenta() -> Style {
        Style::default().fg(Color::Magenta)
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

    pub fn darkgray() -> Style {
        Style::default().fg(Color::DarkGray)
    }

    pub fn highlight(focus: bool) -> Style {
        if focus {
            cyan().not_dim().reversed()
        } else {
            cyan().dim().reversed()
        }
    }
}
