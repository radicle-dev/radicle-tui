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
