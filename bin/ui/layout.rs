use ratatui::layout::{Constraint, Layout};

pub fn page() -> Layout {
    Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
}

pub fn container() -> Layout {
    Layout::vertical([Constraint::Length(3), Constraint::Min(1)])
}

pub fn list_item() -> Layout {
    Layout::horizontal([Constraint::Ratio(1, 3), Constraint::Ratio(2, 3)])
}
