use ratatui::style::{Color, Style};

#[derive(Clone, Default)]
pub enum Mode {
    #[default]
    Light,
    Dark,
}

#[derive(Clone, Debug)]
pub struct Theme {
    border_color: Color,
    focus_border_color: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::default_dark()
    }
}

impl Theme {
    pub fn default_dark() -> Self {
        Self {
            border_color: Color::Indexed(236),
            focus_border_color: Color::Indexed(238),
        }
    }

    pub fn default_light() -> Self {
        Self {
            // border_color: style::gray().fg.unwrap_or_default(),
            // focus_border_color: Color::Reset,
            border_color: Color::Rgb(170, 170, 170),
            focus_border_color: Color::Black,
        }
    }

    pub fn border_style(&self, focus: bool) -> Style {
        if focus {
            Style::default().fg(self.focus_border_color)
        } else {
            Style::default().fg(self.border_color)
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

    pub fn border(focus: bool) -> Style {
        // match mode {
        //     Mode::Light => {
        //         log::warn!("light...");
        //         if focus {
        //             Style::default().fg(Color::Black)
        //         } else {
        //             Style::default().fg(Color::Gray)
        //         }
        //     }
        //     Mode::Dark => {
        //         log::warn!("dark...");
        //         if focus {
        //             Style::default().fg(Color::Indexed(238))
        //         } else {
        //             Style::default().fg(Color::Indexed(236))
        //         }
        //     }
        // }
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
