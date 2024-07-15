use std::collections::HashMap;

use radicle_tui as tui;
use tui::ui::theme::Theme;

static THEME_RADICLE: &str = "Radicle";

pub type ThemeBundleId = String;

/// `ThemeMode` defines which theme is selected from a `ThemeBundle`. It can
/// be either `light``, `dark`` or `auto``, which sets the mode depending on
/// the terminal background luma.
#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum ThemeMode {
    Auto,
    Light,
    Dark,
}

/// A `ThemeBundle` defines a tuple of themes, that should be adapted to light or
/// dark terminal colors.
#[derive(Debug)]
pub struct ThemeBundle {
    pub light: Theme,
    pub dark: Theme,
}

impl Default for ThemeBundle {
    fn default() -> Self {
        Self {
            light: Theme::default_light(),
            dark: Theme::default_dark(),
        }
    }
}

#[derive(Debug)]
pub struct ThemeSettings {
    /// Set light or dark mode, or detect terminal background luma and
    /// switch automatically.
    mode: ThemeMode,
    /// Theme bundle identifier.
    active_bundle: ThemeBundleId,
    /// All theme bundles.
    bundles: HashMap<ThemeBundleId, ThemeBundle>,
}

impl ThemeSettings {
    pub fn mode(&self) -> &ThemeMode {
        &self.mode
    }

    pub fn active_bundle(&self) -> Option<&ThemeBundle> {
        self.bundles.get(&self.active_bundle)
    }
}

#[derive(Debug)]
pub struct Settings {
    pub theme: ThemeSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeSettings {
                mode: ThemeMode::Auto,
                active_bundle: THEME_RADICLE.into(),
                bundles: HashMap::from([(THEME_RADICLE.to_string(), ThemeBundle::default())]),
            },
        }
    }
}
