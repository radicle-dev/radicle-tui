use ratatui::style::{Style, Stylize};
use ratatui::text::Span;

use crate::flux::ui::theme::style;

pub fn blank() -> Span<'static> {
    Span::styled("", Style::default())
}

pub fn default(content: String) -> Span<'static> {
    Span::styled(content, Style::default())
}

pub fn primary(content: String) -> Span<'static> {
    default(content).style(style::cyan())
}

pub fn secondary(content: String) -> Span<'static> {
    default(content).style(style::magenta())
}

pub fn ternary(content: String) -> Span<'static> {
    default(content).style(style::blue())
}

pub fn positive(content: String) -> Span<'static> {
    default(content).style(style::green())
}

pub fn negative(content: String) -> Span<'static> {
    default(content).style(style::red())
}

pub fn badge(content: String) -> Span<'static> {
    let content = &format!(" {content} ");
    default(content.to_string()).magenta().reversed()
}

pub fn alias(content: String) -> Span<'static> {
    secondary(content)
}

pub fn labels(content: String) -> Span<'static> {
    ternary(content)
}

pub fn timestamp(content: String) -> Span<'static> {
    default(content).style(style::gray().dim())
}

pub fn notification_id(content: String) -> Span<'static> {
    default(content).style(style::gray().dim())
}

pub fn notification_type(content: String) -> Span<'static> {
    default(content).style(style::gray().dim())
}
