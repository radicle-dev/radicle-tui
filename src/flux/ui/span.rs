use ratatui::style::{Style, Stylize};
use ratatui::text::Text;

use crate::flux::ui::theme::style;

pub fn blank() -> Text<'static> {
    Text::styled("", Style::default())
}

pub fn default(content: String) -> Text<'static> {
    Text::styled(content, Style::default())
}

pub fn primary(content: String) -> Text<'static> {
    default(content).style(style::cyan())
}

pub fn secondary(content: String) -> Text<'static> {
    default(content).style(style::magenta())
}

pub fn positive(content: String) -> Text<'static> {
    default(content).style(style::green())
}

pub fn badge(content: String) -> Text<'static> {
    let content = &format!(" {content} ");
    default(content.to_string()).magenta().reversed()
}

pub fn timestamp(content: String) -> Text<'static> {
    default(content).style(style::gray().dim())
}
