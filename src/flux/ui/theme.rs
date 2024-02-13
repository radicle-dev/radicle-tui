pub mod style {
    use ratatui::style::{Color, Modifier, Style};

    pub fn reset() -> Style {
        Style::default().fg(Color::Reset)
    }

    pub fn reset_dim() -> Style {
        Style::default()
            .fg(Color::Reset)
            .add_modifier(Modifier::DIM)
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
        yellow().add_modifier(Modifier::DIM)
    }

    pub fn yellow_dim_reversed() -> Style {
        yellow_dim().add_modifier(Modifier::REVERSED)
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
            .add_modifier(Modifier::DIM)
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
        Style::default().fg(Color::Gray).add_modifier(Modifier::DIM)
    }

    pub fn darkgray() -> Style {
        Style::default().fg(Color::DarkGray)
    }

    pub fn reversed() -> Style {
        Style::default().add_modifier(Modifier::REVERSED)
    }

    pub fn default_reversed() -> Style {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::REVERSED)
    }

    pub fn magenta_reversed() -> Style {
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::REVERSED)
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
            .add_modifier(Modifier::REVERSED)
    }
}
