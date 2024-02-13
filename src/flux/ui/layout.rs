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
