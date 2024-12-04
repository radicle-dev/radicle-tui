use std::path::Path;

use ratatui::prelude::Stylize;
use ratatui::text::Span;

use crate::cob::HunkState;

use radicle_tui as tui;

use tui::ui::span;

pub fn hunk_state(state: &HunkState) -> Span<'static> {
    match state {
        HunkState::Accepted => span::positive("✓"),
        // HunkState::Rejected => span::secondary("?"),
        HunkState::Rejected => span::negative("✗"),
    }
}

pub fn pretty_path(path: &Path, crossed_out: bool, show_path: bool) -> Vec<Span<'static>> {
    let file = path.file_name().unwrap_or_default();
    let path = if path.iter().count() > 1 {
        path.iter()
            .take(path.iter().count() - 1)
            .map(|component| component.to_string_lossy().to_string())
            .collect::<Vec<_>>()
    } else {
        vec![]
    };

    let mut spans = vec![];

    let filename = if crossed_out {
        span::default(file.to_string_lossy().as_ref()).crossed_out()
    } else {
        span::default(file.to_string_lossy().as_ref())
    };
    spans.push(filename);

    if show_path {
        spans.extend([
            span::default(" "),
            span::default(&path.join(&String::from("/")).to_string()).dark_gray(),
        ]);
    }

    spans
}
