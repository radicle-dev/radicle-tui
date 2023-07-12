use tuirealm::props::{AttrValue, Attribute};
use tuirealm::tui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::MockComponent;

pub struct AppHeader {
    pub nav: Rect,
    pub info: Rect,
    pub line: Rect,
}

pub struct DefaultPage {
    pub navigation: Rect,
    pub component: Rect,
    pub context: Rect,
    pub shortcuts: Rect,
}

pub struct IssuePage {
    pub header: Rect,
    pub left: Rect,
    pub right: Rect,
    pub shortcuts: Rect,
}

pub fn v_stack(
    widgets: Vec<Box<dyn MockComponent>>,
    area: Rect,
) -> Vec<(Box<dyn MockComponent>, Rect)> {
    let constraints = widgets
        .iter()
        .map(|w| {
            Constraint::Length(
                w.query(Attribute::Height)
                    .unwrap_or(AttrValue::Size(0))
                    .unwrap_size(),
            )
        })
        .collect::<Vec<_>>();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    widgets.into_iter().zip(layout.into_iter()).collect()
}

pub fn h_stack(
    widgets: Vec<Box<dyn MockComponent>>,
    area: Rect,
) -> Vec<(Box<dyn MockComponent>, Rect)> {
    let constraints = widgets
        .iter()
        .map(|w| {
            Constraint::Length(
                w.query(Attribute::Width)
                    .unwrap_or(AttrValue::Size(0))
                    .unwrap_size(),
            )
        })
        .collect::<Vec<_>>();
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    widgets.into_iter().zip(layout.into_iter()).collect()
}

pub fn app_header(area: Rect, info_w: u16) -> AppHeader {
    let nav_w = area.width.saturating_sub(info_w);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(nav_w), Constraint::Length(info_w)].as_ref())
        .split(layout[1]);

    AppHeader {
        nav: top[0],
        info: top[1],
        line: layout[2],
    }
}

pub fn default_page(area: Rect, shortcuts_h: u16) -> DefaultPage {
    let nav_h = 3u16;
    let context_h = 1u16;
    let margin_h = 1u16;
    let component_h = area
        .height
        .saturating_sub(nav_h.saturating_add(context_h).saturating_add(shortcuts_h));

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .horizontal_margin(margin_h)
        .constraints(
            [
                Constraint::Length(nav_h),
                Constraint::Length(component_h),
                Constraint::Length(context_h),
                Constraint::Length(shortcuts_h),
            ]
            .as_ref(),
        )
        .split(area);

    DefaultPage {
        navigation: layout[0],
        component: layout[1],
        context: layout[2],
        shortcuts: layout[3],
    }
}

pub fn headerless_page(area: Rect) -> Vec<Rect> {
    let margin_h = 1u16;
    let content_h = area.height.saturating_sub(margin_h);

    Layout::default()
        .direction(Direction::Vertical)
        .horizontal_margin(margin_h)
        .constraints([Constraint::Length(content_h)].as_ref())
        .split(area)
}

pub fn root_component_with_context(area: Rect, context_h: u16, shortcuts_h: u16) -> Vec<Rect> {
    let content_h = area
        .height
        .saturating_sub(shortcuts_h.saturating_add(context_h));

    Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(content_h),
                Constraint::Length(context_h),
                Constraint::Length(shortcuts_h),
            ]
            .as_ref(),
        )
        .split(area)
}

pub fn centered_label(label_w: u16, area: Rect) -> Rect {
    let label_h = 1u16;
    let spacer_w = area.width.saturating_sub(label_w).saturating_div(2);
    let spacer_h = area.height.saturating_sub(label_h).saturating_div(2);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(spacer_h),
                Constraint::Length(label_h),
                Constraint::Length(spacer_h),
            ]
            .as_ref(),
        )
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Length(spacer_w),
                Constraint::Length(label_w),
                Constraint::Length(spacer_w),
            ]
            .as_ref(),
        )
        .split(layout[1])[1]
}

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}

pub fn issue_page(area: Rect, shortcuts_h: u16) -> IssuePage {
    let content_h = area.height.saturating_sub(shortcuts_h);
    let header_h = 3u16;

    let root = Layout::default()
        .direction(Direction::Vertical)
        .horizontal_margin(1)
        .constraints(
            [
                Constraint::Length(header_h),
                Constraint::Length(content_h),
                Constraint::Length(shortcuts_h),
            ]
            .as_ref(),
        )
        .split(area);

    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(root[1]);

    IssuePage {
        header: root[0],
        left: split[0],
        right: split[1],
        shortcuts: root[2],
    }
}
