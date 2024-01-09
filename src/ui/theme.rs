use tuirealm::props::Color;

// const COLOR_DEFAULT_FG: Color = Color::Rgb(200, 200, 200);
// const COLOR_DEFAULT_DARK_FG: Color = Color::Rgb(150, 150, 150);
const COLOR_DEFAULT_DARK: Color = Color::Rgb(100, 100, 100);
const COLOR_DEFAULT_DARKER: Color = Color::Rgb(70, 70, 70);
const COLOR_DEFAULT_FAINT: Color = Color::Rgb(20, 20, 20);

#[derive(Debug, Clone)]
pub struct Colors {
    // pub default_fg: Color,
    // pub tabs_highlighted_fg: Color,
    // pub app_header_project_fg: Color,
    // pub app_header_rid_fg: Color,
    pub labeled_container_bg: Color,
    pub item_list_highlighted_bg: Color,
    // pub property_name_fg: Color,
    // pub property_divider_fg: Color,
    pub shortcut_short_fg: Color,
    pub shortcut_long_fg: Color,
    pub shortcutbar_divider_fg: Color,
    // pub browser_list_id: Color,
    // pub browser_list_title: Color,
    // pub browser_list_description: Color,
    // pub browser_list_author: Color,
    // pub browser_list_labels: Color,
    // pub browser_list_comments: Color,
    // pub browser_list_timestamp: Color,
    // pub browser_patch_list_head: Color,
    // pub browser_patch_list_added: Color,
    // pub browser_patch_list_removed: Color,
    // pub context_bg: Color,
    // pub context_light: Color,
    // pub context_dark: Color,
    // pub context_badge_bg: Color,
    // pub context_badge_edit_bg: Color,
    // pub context_color_fg: Color,
    // pub container_border_fg: Color,
    // pub container_border_focus_fg: Color,
    pub input_placeholder_fg: Color,
}

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

/// The Radicle TUI theme. Will be defined in a JSON config file in the
/// future. e.g.:
/// {
///     "name": "Default",
///     "colors": {
///         "foreground": "#ffffff",
///         "propertyForeground": "#ffffff",
///         "highlightedBackground": "#000000",
///     },
///     "icons": {
///         "workspaces.divider": "|",
///         "shortcuts.divider: "∙",
///     }
/// }
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub colors: Colors,
    pub icons: Icons,
    pub tables: Tables,
}

pub fn default_dark() -> Theme {
    Theme {
        name: String::from("Default"),
        colors: Colors {
            // default_fg: COLOR_DEFAULT_FG,
            // tabs_highlighted_fg: Color::Magenta,
            // app_header_project_fg: Color::Cyan,
            // app_header_rid_fg: Color::Yellow,
            labeled_container_bg: COLOR_DEFAULT_FAINT,
            item_list_highlighted_bg: Color::DarkGray,
            // property_name_fg: Color::Cyan,
            // property_divider_fg: COLOR_DEFAULT_DARK,
            shortcut_short_fg: COLOR_DEFAULT_DARK,
            shortcut_long_fg: COLOR_DEFAULT_DARKER,
            shortcutbar_divider_fg: COLOR_DEFAULT_DARKER,
            // browser_list_id: Color::Cyan,
            // browser_list_title: COLOR_DEFAULT_FG,
            // browser_list_description: COLOR_DEFAULT_DARK,
            // browser_list_author: Color::Magenta,
            // browser_list_labels: Color::LightBlue,
            // browser_list_comments: COLOR_DEFAULT_DARK_FG,
            // browser_list_timestamp: COLOR_DEFAULT_DARK,
            // browser_patch_list_head: Color::LightBlue,
            // browser_patch_list_added: Color::Green,
            // browser_patch_list_removed: Color::Red,
            // context_bg: Color::DarkGray,
            // context_light: Color::Gray,
            // context_dark: COLOR_DEFAULT_DARK,
            // context_badge_bg: Color::Magenta,
            // context_badge_edit_bg: Color::Red,
            // context_color_fg: Color::Cyan,
            // container_border_fg: Color::Black,
            // container_border_focus_fg: Color::DarkGray,
            input_placeholder_fg: COLOR_DEFAULT_DARK,
        },
        icons: Icons {
            property_divider: '∙',
            shortcutbar_divider: '∙',
            tab_divider: '|',
            tab_overline: '▔',
            whitespace: ' ',
        },
        tables: Tables { spacing: 2 },
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

    pub fn darkgray() -> Style {
        Style::default().fg(Color::DarkGray)
    }

    pub fn default_reversed() -> Style {
        Style::default()
            .fg(Color::Reset)
            .bg(Color::DarkGray)
            .add_modifier(TextModifiers::DIM)
    }

    pub fn magenta_reversed() -> Style {
        Style::default().fg(Color::DarkGray).bg(Color::Magenta)
    }

    pub fn yellow_reversed() -> Style {
        Style::default().fg(Color::DarkGray).bg(Color::Yellow)
    }
}
