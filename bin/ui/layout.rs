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

pub mod ext {
    use radicle_tui::ui::im::Layout;

    pub fn popup(percent_x: u16, percent_y: u16) -> Layout {
        Layout::Popup {
            percent_x,
            percent_y,
        }
    }
}
