use tui_realm_stdlib::Phantom;

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::props::{AttrValue, Attribute, Color, Props, Style};
use tuirealm::tui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::tui::widgets::Block;
use tuirealm::{Frame, MockComponent, State, StateValue};

use super::layout;
use super::widget::{Widget, WidgetComponent};

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

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

/// Some user events need to be handled globally (e.g. user presses key `q` to quit
/// the application). This component can be used in conjunction with SubEventClause
/// to handle those events.
#[derive(Default, MockComponent)]
pub struct GlobalPhantom {
    component: Phantom,
}

/// A label that can be styled using a foreground color and text modifiers.
/// Its height is fixed, its width depends on the length of the text it displays.
#[derive(Clone)]
pub struct Label {
    content: StateValue,
}

impl Label {
    pub fn new(content: StateValue) -> Self {
        Self { content }
    }
}

impl WidgetComponent for Label {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        use tui_realm_stdlib::Label;

        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();
        let foreground = properties
            .get_or(Attribute::Foreground, AttrValue::Color(Color::Reset))
            .unwrap_color();
        let background = properties
            .get_or(Attribute::Background, AttrValue::Color(Color::Reset))
            .unwrap_color();

        if display {
            let mut label = match properties.get(Attribute::TextProps) {
                Some(modifiers) => Label::default()
                    .foreground(foreground)
                    .background(background)
                    .modifiers(modifiers.unwrap_text_modifiers())
                    .text(self.content.clone().unwrap_string()),
                None => Label::default()
                    .foreground(foreground)
                    .background(background)
                    .text(self.content.clone().unwrap_string()),
            };

            label.view(frame, area);
        }
    }

    fn state(&self) -> State {
        State::One(self.content.clone())
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

/// A labeled container header.
#[derive(Clone)]
pub struct ContainerHeader {
    content: Widget<Label>,
    spacer: Widget<Label>,
}

impl ContainerHeader {
    pub fn new(content: Widget<Label>, spacer: Widget<Label>) -> Self {
        Self { content, spacer }
    }
}

impl WidgetComponent for ContainerHeader {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();

        if display {
            let labels: Vec<Box<dyn MockComponent>> = vec![
                self.content.clone().to_boxed(),
                self.spacer.clone().to_boxed(),
            ];

            let layout = layout::h_stack(labels, area);
            for (mut shortcut, area) in layout {
                shortcut.view(frame, area);
            }
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

pub struct LabeledContainer {
    header: Widget<ContainerHeader>,
    component: Box<dyn MockComponent>,
}

impl LabeledContainer {
    pub fn new(header: Widget<ContainerHeader>, component: Box<dyn MockComponent>) -> Self {
        Self { header, component }
    }
}

impl WidgetComponent for LabeledContainer {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();
        let background = properties
            .get_or(Attribute::Background, AttrValue::Color(Color::Reset))
            .unwrap_color();
        let header_height = self
            .header
            .query(Attribute::Height)
            .unwrap_or(AttrValue::Size(1))
            .unwrap_size();

        if display {
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(header_height), Constraint::Length(0)].as_ref())
                .split(area);

            self.header.view(frame, layout[0]);

            // Make some space on the left
            let inner_layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(vec![Constraint::Length(1), Constraint::Min(0)].as_ref())
                .split(layout[1]);
            // reverse draw order: child needs to be drawn first?
            self.component.view(frame, inner_layout[1]);

            let block = Block::default().style(Style::default().bg(background));
            frame.render_widget(block, layout[1]);
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        self.component.perform(cmd)
    }
}

/// A shortcut that consists of a label displaying the "hotkey", a label that displays
/// the action and a spacer between them.
#[derive(Clone)]
pub struct Shortcut {
    short: Widget<Label>,
    divider: Widget<Label>,
    long: Widget<Label>,
}

impl Shortcut {
    pub fn new(short: Widget<Label>, divider: Widget<Label>, long: Widget<Label>) -> Self {
        Self {
            short,
            divider,
            long,
        }
    }
}

impl WidgetComponent for Shortcut {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();

        if display {
            let labels: Vec<Box<dyn MockComponent>> = vec![
                self.short.clone().to_boxed(),
                self.divider.clone().to_boxed(),
                self.long.clone().to_boxed(),
            ];

            let layout = layout::h_stack(labels, area);
            for (mut shortcut, area) in layout {
                shortcut.view(frame, area);
            }
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

/// A shortcut bar that displays multiple shortcuts and separates them with a
/// divider.
pub struct ShortcutBar {
    shortcuts: Vec<Widget<Shortcut>>,
    divider: Widget<Label>,
}

impl ShortcutBar {
    pub fn new(shortcuts: Vec<Widget<Shortcut>>, divider: Widget<Label>) -> Self {
        Self { shortcuts, divider }
    }
}

impl WidgetComponent for ShortcutBar {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();

        if display {
            let mut widgets: Vec<Box<dyn MockComponent>> = vec![];
            let mut shortcuts = self.shortcuts.iter_mut().peekable();

            while let Some(shortcut) = shortcuts.next() {
                if shortcuts.peek().is_some() {
                    widgets.push(shortcut.clone().to_boxed());
                    widgets.push(self.divider.clone().to_boxed())
                } else {
                    widgets.push(shortcut.clone().to_boxed());
                }
            }

            let layout = layout::h_stack(widgets, area);
            for (mut widget, area) in layout {
                widget.view(frame, area);
            }
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

/// A component that displays a labeled property.
#[derive(Clone)]
pub struct Property {
    label: Widget<Label>,
    divider: Widget<Label>,
    property: Widget<Label>,
}

impl Property {
    pub fn new(label: Widget<Label>, divider: Widget<Label>, property: Widget<Label>) -> Self {
        Self {
            label,
            divider,
            property,
        }
    }
}

impl WidgetComponent for Property {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();

        if display {
            let labels: Vec<Box<dyn MockComponent>> = vec![
                self.label.clone().to_boxed(),
                self.divider.clone().to_boxed(),
                self.property.clone().to_boxed(),
            ];

            let layout = layout::h_stack(labels, area);
            for (mut label, area) in layout {
                label.view(frame, area);
            }
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

/// A component that can display lists of labeled properties
#[derive(Default)]
#[allow(clippy::vec_box)]
pub struct PropertyList {
    properties: Vec<Widget<Property>>,
}

impl PropertyList {
    pub fn new(properties: Vec<Widget<Property>>) -> Self {
        Self { properties }
    }
}

impl WidgetComponent for PropertyList {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();

        if display {
            let properties = self
                .properties
                .iter()
                .map(|property| property.clone().to_boxed() as Box<dyn MockComponent>)
                .collect();

            let layout = layout::v_stack(properties, area);
            for (mut property, area) in layout {
                property.view(frame, area);
            }
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}
