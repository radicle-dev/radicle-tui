use ratatui::style::{Style, Stylize};
use ratatui::text::Span;

use crate::ui::theme::style;

pub fn blank() -> Span<'static> {
    Span::styled("", Style::default())
}

pub fn default(content: &str) -> Span<'static> {
    Span::styled(content.to_string(), Style::default())
}

pub fn primary(content: &str) -> Span<'static> {
    default(content).style(style::cyan())
}

pub fn secondary(content: &str) -> Span<'static> {
    default(content).style(style::magenta())
}

pub fn ternary(content: &str) -> Span<'static> {
    default(content).style(style::blue())
}

pub fn positive(content: &str) -> Span<'static> {
    default(content).style(style::green())
}

pub fn negative(content: &str) -> Span<'static> {
    default(content).style(style::red())
}

pub fn badge(content: &str) -> Span<'static> {
    let content = &format!(" {content} ");
    default(content).magenta().reversed()
}

pub fn alias(content: &str) -> Span<'static> {
    secondary(content)
}

pub fn labels(content: &str) -> Span<'static> {
    ternary(content)
}

pub fn timestamp(content: &str) -> Span<'static> {
    default(content).style(style::gray().dim())
}

pub fn notification_id(content: &str) -> Span<'static> {
    default(content).style(style::gray().dim())
}

pub fn notification_type(content: &str) -> Span<'static> {
    default(content).style(style::gray().dim())
}

pub fn step(step: usize, len: usize, fill_zeros: bool) -> Span<'static> {
    if fill_zeros {
        if len > 10 {
            badge(&format!("{:-02}/{:-02}", step, len))
        } else if len > 100 {
            badge(&format!("{:-03}/{:-03}", step, len))
        } else if len > 1000 {
            badge(&format!("{:-04}/{:-04}", step, len))
        } else if len > 10000 {
            badge(&format!("{:-05}/{:-05}", step, len))
        } else {
            badge(&format!("{}/{}", step, len))
        }
    } else {
        badge(&format!("{}/{}", step, len))
    }
}

pub fn progress(step: usize, len: usize) -> Span<'static> {
    let progress = step as f32 / len as f32 * 100_f32;
    let progress = progress as usize;
    default(&format!("{}%", progress)).dim()
}
