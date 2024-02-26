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

pub fn progress(step: usize, len: usize, fill_zeros: bool) -> Span<'static> {
    if fill_zeros {
        if len > 10 {
            badge(format!("{:-02}/{:-02}", step, len))
        } else if len > 100 {
            badge(format!("{:-03}/{:-03}", step, len))
        } else if len > 1000 {
            badge(format!("{:-04}/{:-04}", step, len))
        } else if len > 10000 {
            badge(format!("{:-05}/{:-05}", step, len))
        } else {
            badge(format!("{}/{}", step, len))
        }
    } else {
        badge(format!("{}/{}", step, len))
    }
}
