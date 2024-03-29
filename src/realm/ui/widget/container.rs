use tuirealm::command::{Cmd, CmdResult};
use tuirealm::props::{AttrValue, Attribute, BorderSides, Props, TextModifiers};
use tuirealm::tui::layout::{Constraint, Direction, Layout, Margin, Rect};
use tuirealm::tui::widgets::{Block, Cell, Clear, Row};
use tuirealm::{Frame, MockComponent, State, StateValue};

use crate::realm::ui::ext::HeaderBlock;
use crate::realm::ui::layout;
use crate::realm::ui::state::TabState;
use crate::realm::ui::theme::{style, Theme};
use crate::realm::ui::widget::{utils, Widget, WidgetComponent};

use super::label::Label;
use super::list::ColumnWidth;

/// Some user events need to be handled globally (e.g. user presses key `q` to quit
/// the application). This component can be used in conjunction with SubEventClause
/// to handle those events.
#[derive(Default)]
pub struct GlobalListener {}

impl WidgetComponent for GlobalListener {
    fn view(&mut self, _properties: &Props, _frame: &mut Frame, _area: Rect) {}

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

/// A vertical separator.
#[derive(Clone)]
pub struct VerticalLine {
    line: Widget<Label>,
}

impl VerticalLine {
    pub fn new(line: Widget<Label>) -> Self {
        Self { line }
    }
}

impl WidgetComponent for VerticalLine {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();

        if display {
            // Repeat and render line.
            let overlines = vec![self.line.clone(); area.width as usize];
            let overlines = overlines
                .iter()
                .map(|l| l.clone().to_boxed() as Box<dyn MockComponent>)
                .collect();
            let line_layout = layout::h_stack(overlines, area);
            for (mut line, area) in line_layout {
                line.view(frame, area);
            }
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

/// A tab header that displays all labels horizontally aligned and separated
/// by a divider. Highlights the label defined by the current tab index.
#[derive(Clone)]
pub struct Tabs {
    tabs: Vec<Widget<Label>>,
    state: TabState,
}

impl Tabs {
    pub fn new(tabs: Vec<Widget<Label>>) -> Self {
        let count = &tabs.len();
        Self {
            tabs,
            state: TabState {
                selected: 0,
                len: *count as u16,
            },
        }
    }
}

impl WidgetComponent for Tabs {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let selected = self.state().unwrap_one().unwrap_u16();
        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();

        if display {
            // Render tabs, highlighting the selected tab.
            let mut tabs = vec![];
            for (index, tab) in self.tabs.iter().enumerate() {
                let mut tab = tab.clone().to_boxed();
                if index == selected as usize {
                    tab.attr(
                        Attribute::TextProps,
                        AttrValue::TextModifiers(TextModifiers::REVERSED),
                    );
                }
                tabs.push(tab.clone().to_boxed() as Box<dyn MockComponent>);
            }
            tabs.push(Widget::new(Label).to_boxed());

            let tab_layout = layout::h_stack(tabs, area);
            for (mut tab, area) in tab_layout {
                tab.view(frame, area);
            }
        }
    }

    fn state(&self) -> State {
        State::One(StateValue::U16(self.state.selected))
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        use tuirealm::command::Direction;

        match cmd {
            Cmd::Move(Direction::Right) => {
                let prev = self.state.selected;
                self.state.incr_tab_index(true);
                if prev != self.state.selected {
                    CmdResult::Changed(self.state())
                } else {
                    CmdResult::None
                }
            }
            _ => CmdResult::None,
        }
    }
}

/// An application info widget that renders project / branch information
/// and a separator line. Used in conjunction with [`Tabs`].
pub struct AppInfo {
    project: Widget<Label>,
    rid: Widget<Label>,
}

impl AppInfo {
    pub fn new(project: Widget<Label>, rid: Widget<Label>) -> Self {
        Self { project, rid }
    }
}

impl WidgetComponent for AppInfo {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();

        let project_w = self
            .project
            .query(Attribute::Width)
            .unwrap_or(AttrValue::Size(10))
            .unwrap_size();

        let rid_w = self
            .rid
            .query(Attribute::Width)
            .unwrap_or(AttrValue::Size(10))
            .unwrap_size();

        if display {
            let layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(vec![
                    Constraint::Length(project_w),
                    Constraint::Length(rid_w),
                ])
                .split(area);

            self.project.view(frame, layout[0]);
            self.rid.view(frame, layout[1]);
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

/// A common application header that renders project / branch
/// information and an optional navigation.
pub struct AppHeader {
    nav: Option<Widget<Tabs>>,
    info: Widget<AppInfo>,
    line: Widget<VerticalLine>,
}

impl AppHeader {
    pub fn new(
        nav: Option<Widget<Tabs>>,
        info: Widget<AppInfo>,
        line: Widget<VerticalLine>,
    ) -> Self {
        Self { nav, info, line }
    }
}

impl WidgetComponent for AppHeader {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();
        let info_w = self
            .info
            .query(Attribute::Width)
            .unwrap_or(AttrValue::Size(10))
            .unwrap_size();

        if display {
            let layout = layout::app_header(area, info_w);

            if let Some(nav) = self.nav.as_mut() {
                nav.view(frame, layout.nav);
            }
            self.info.view(frame, layout.info);
            self.line.view(frame, layout.line);
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        self.nav
            .as_mut()
            .map(|nav| nav.perform(cmd))
            .unwrap_or(CmdResult::None)
    }
}

/// A labeled container header.
pub struct Header<const W: usize> {
    header: [Widget<Label>; W],
    widths: [ColumnWidth; W],
    theme: Theme,
}

impl<const W: usize> Header<W> {
    pub fn new(header: [Widget<Label>; W], widths: [ColumnWidth; W], theme: Theme) -> Self {
        Self {
            header,
            widths,
            theme,
        }
    }
}

impl<const W: usize> WidgetComponent for Header<W> {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        if display {
            let block = HeaderBlock::default()
                .borders(BorderSides::all())
                .border_style(style::border(focus))
                .border_type(self.theme.border_type);
            frame.render_widget(block, area);

            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints(vec![Constraint::Min(1)])
                .vertical_margin(1)
                .horizontal_margin(1)
                .split(area);

            let widths = utils::column_widths(area, &self.widths, self.theme.tables.spacing);
            let header: [Cell; W] = self
                .header
                .iter()
                .map(|label| {
                    let cell: Cell = label.into();
                    cell.style(style::reset())
                })
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let header: Row<'_> = Row::new(header);

            let table = tuirealm::tui::widgets::Table::new(vec![])
                .column_spacing(self.theme.tables.spacing)
                .header(header)
                .widths(&widths);
            frame.render_widget(table, layout[0]);
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

pub struct Container {
    component: Box<dyn MockComponent>,
    theme: Theme,
}

impl Container {
    pub fn new(component: Box<dyn MockComponent>, theme: Theme) -> Self {
        Self { component, theme }
    }
}

impl WidgetComponent for Container {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        if display {
            // Make some space on the left
            let layout = Layout::default()
                .direction(Direction::Horizontal)
                .horizontal_margin(1)
                .vertical_margin(1)
                .constraints(vec![Constraint::Length(1), Constraint::Min(0)])
                .split(area);
            // reverse draw order: child needs to be drawn first?
            self.component
                .attr(Attribute::Focus, AttrValue::Flag(focus));
            self.component.view(frame, layout[1]);

            let block = Block::default()
                .borders(BorderSides::ALL)
                .border_style(style::border(focus))
                .border_type(self.theme.border_type);
            frame.render_widget(block, area);
        }
    }

    fn state(&self) -> State {
        self.component.state()
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        self.component.perform(cmd)
    }
}

pub struct LabeledContainer {
    header: Widget<Header<1>>,
    component: Box<dyn MockComponent>,
    theme: Theme,
}

impl LabeledContainer {
    pub fn new(header: Widget<Header<1>>, component: Box<dyn MockComponent>, theme: Theme) -> Self {
        Self {
            header,
            component,
            theme,
        }
    }
}

impl WidgetComponent for LabeledContainer {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        let header_height = self
            .header
            .query(Attribute::Height)
            .unwrap_or(AttrValue::Size(3))
            .unwrap_size();

        if display {
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(header_height), Constraint::Min(1)].as_ref())
                .split(area);

            self.header.attr(Attribute::Focus, AttrValue::Flag(focus));
            self.header.view(frame, layout[0]);

            let block = Block::default()
                .borders(BorderSides::BOTTOM | BorderSides::LEFT | BorderSides::RIGHT)
                .border_style(style::border(focus))
                .border_type(self.theme.border_type);
            frame.render_widget(block.clone(), layout[1]);

            self.component
                .attr(Attribute::Focus, AttrValue::Flag(focus));
            self.component.view(
                frame,
                block.inner(layout[1]).inner(&Margin {
                    vertical: 0,
                    horizontal: 1,
                }),
            );
        }
    }

    fn state(&self) -> State {
        self.component.state()
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        self.component.perform(cmd)
    }
}

pub struct Popup {
    component: Widget<LabeledContainer>,
}

impl Popup {
    pub fn new(_theme: Theme, component: Widget<LabeledContainer>) -> Self {
        Self { component }
    }
}

impl WidgetComponent for Popup {
    fn view(&mut self, properties: &Props, frame: &mut Frame, _area: Rect) {
        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();
        let width = properties
            .get_or(Attribute::Width, AttrValue::Size(50))
            .unwrap_size();
        let height = properties
            .get_or(Attribute::Height, AttrValue::Size(50))
            .unwrap_size();

        if display {
            let size = frame.size();

            let area = layout::centered_rect(width, height, size);
            frame.render_widget(Clear, area);

            self.component
                .attr(Attribute::Focus, AttrValue::Flag(focus));
            self.component.view(frame, area);
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        self.component.perform(cmd)
    }
}
