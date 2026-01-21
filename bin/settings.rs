use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use lazy_static::lazy_static;

use thiserror::Error;

use homedir::my_home;

use radicle_tui as tui;

use tui::ui::theme::Theme;

static THEME_RADICLE: &str = "Radicle";

#[derive(Clone, Debug)]
pub struct TerminalInfo {
    pub luma: Option<f32>,
}

impl TerminalInfo {
    pub fn is_dark(&self) -> bool {
        self.luma.unwrap_or_default() <= 0.6
    }
}

lazy_static! {
    static ref TERMINAL_INFO: TerminalInfo = TerminalInfo {
        luma: Some(terminal_light::luma().unwrap_or_default())
    };
}

pub type ThemeBundleId = String;

#[derive(Error, Debug)]
pub enum Error {
    #[error(
        "could not resolve home directory ($HOME is not set and `/etc/passwd` does not resolve)"
    )]
    Home,
}

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

pub fn configure_theme(settings: &Settings) -> Theme {
    let default_bundle = ThemeBundle::default();
    let theme_bundle = settings.theme.active_bundle().unwrap_or(&default_bundle);

    match settings.theme.mode() {
        ThemeMode::Auto => {
            if TERMINAL_INFO.is_dark() {
                theme_bundle.dark.clone()
            } else {
                theme_bundle.light.clone()
            }
        }
        ThemeMode::Light => theme_bundle.light.clone(),
        ThemeMode::Dark => theme_bundle.dark.clone(),
    }
}

pub fn get_state_path() -> Result<PathBuf, Error> {
    let base = match env::var("XDG_STATE_HOME") {
        Ok(path) => PathBuf::from(path),
        Err(err) => {
            log::debug!("Could not read `XDG_STATE_HOME`: {err}");
            my_home()
                .ok()
                .flatten()
                .ok_or(Error::Home)?
                .join(".local/state")
        }
    };

    Ok(base.join("radicle-tui"))
}
