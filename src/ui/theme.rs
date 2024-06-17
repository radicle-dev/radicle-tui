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

    pub fn border(focus: bool) -> Style {
        if focus {
            Style::default().fg(Color::Indexed(238))
        } else {
            Style::default().fg(Color::Indexed(236))
        }
    }

    pub fn highlight(focus: bool) -> Style {
        if focus {
            cyan().not_dim().reversed()
        } else {
            cyan().dim().reversed()
        }
    }
}
