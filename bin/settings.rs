use lazy_static::lazy_static;

use radicle_tui::ui::theme::Theme;
use ratatui::style::Color;

lazy_static! {
    static ref IS_DARK: bool = match terminal_light::luma() {
        Ok(luma) if luma <= 0.6 => true,
        _ => false,
    };
}

#[derive(Clone, Debug)]
pub struct Raw {
    dark_theme: Option<Theme>,
    light_theme: Option<Theme>,
}

impl Default for Raw {
    fn default() -> Self {
        Self {
            light_theme: Some(Theme {
                border_color: Color::Rgb(170, 170, 170),
                focus_border_color: Color::Black,
            }),
            dark_theme: Some(Theme {
                border_color: Color::Indexed(236),
                focus_border_color: Color::Indexed(238),
            }),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Evaluated {
    pub theme: Theme,
}

impl From<Raw> for Evaluated {
    fn from(raw: Raw) -> Self {
        let adaptive = raw.light_theme.is_some() && raw.dark_theme.is_some();
        let theme = if adaptive {
            if *IS_DARK {
                raw.dark_theme.unwrap()
            } else {
                raw.light_theme.unwrap()
            }
        } else {
            if let Some(light_theme) = raw.light_theme {
                light_theme
            } else if let Some(dark_theme) = raw.dark_theme {
                dark_theme
            } else {
                Theme::default()
            }
        };

        Evaluated { theme }
    }
}
