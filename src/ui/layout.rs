use ratatui::layout::{Constraint, Direction, Layout, Rect};
pub struct DefaultPage {
    pub component: Rect,
    pub context: Rect,
    pub shortcuts: Rect,
}

pub fn default_page(area: Rect, context_h: u16, shortcuts_h: u16) -> DefaultPage {
    let margin_h = 1u16;
    let component_h = area
        .height
        .saturating_sub(context_h.saturating_add(shortcuts_h));

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .horizontal_margin(margin_h)
        .constraints(
            [
                Constraint::Length(component_h),
                Constraint::Length(context_h),
                Constraint::Length(shortcuts_h),
            ]
            .as_ref(),
        )
        .split(area);

    DefaultPage {
        component: layout[0],
        context: layout[1],
        shortcuts: layout[2],
    }
}

pub fn centered_rect(r: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(layout[1])[1]
}

pub fn fill() -> Layout {
    Layout::vertical([Constraint::Fill(1)].to_vec())
}
