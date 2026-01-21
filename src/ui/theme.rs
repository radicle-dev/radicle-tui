use serde::{Deserialize, Serialize};

use ratatui::style::{Color, Style, Stylize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Theme {
    pub border_style: Style,
    pub focus_border_style: Style,
    pub shortcuts_keys_style: Style,
    pub shortcuts_action_style: Style,
    pub textview_style: Style,
    pub textview_scroll_style: Style,
    pub textview_focus_scroll_style: Style,
    pub bar_on_black_style: Style,
    pub dim_no_focus: bool,
}

impl Default for Theme {
    fn default() -> Self {
        Self::default_dark()
    }
}

impl Theme {
    pub fn default_light() -> Self {
        Self {
            border_style: Style::default().fg(Color::Rgb(170, 170, 170)),
            focus_border_style: Style::default(),
            shortcuts_keys_style: style::yellow(),
            shortcuts_action_style: style::reset(),
            textview_style: style::reset(),
            textview_scroll_style: style::cyan().dim(),
            textview_focus_scroll_style: style::cyan(),
            bar_on_black_style: Style::default().on_gray(),
            dim_no_focus: false,
        }
    }

    pub fn default_dark() -> Self {
        Self {
            border_style: Style::default().fg(Color::Indexed(240)),
            focus_border_style: Style::default().fg(Color::Indexed(246)),
            shortcuts_keys_style: style::yellow().dim(),
            shortcuts_action_style: style::gray().dim(),
            textview_style: style::reset(),
            textview_scroll_style: style::cyan().dim(),
            textview_focus_scroll_style: style::cyan(),
            bar_on_black_style: Style::default().on_black(),
            dim_no_focus: false,
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
