use tuirealm::command::{Cmd, CmdResult};
use tuirealm::props::{AttrValue, Attribute, Props};
use tuirealm::tui::layout::Rect;
use tuirealm::{Frame, MockComponent, State};

use super::label::Label;

use crate::ui::layout;
use crate::ui::theme::Theme;
use crate::ui::widget::{Widget, WidgetComponent};

pub enum Progress {
    Percentage(usize),
    Step(usize, usize),
    None,
}

impl ToString for Progress {
    fn to_string(&self) -> std::string::String {
        match self {
            Progress::Percentage(value) => format!("{value} %"),
            Progress::Step(step, total) => format!("{step}/{total}"),
            _ => String::new(),
        }
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

    fn perform(&mut self, _properties: &Props, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

/// A shortcut bar that displays multiple shortcuts and separates them with a
/// divider.
#[derive(Clone)]
pub struct Shortcuts {
    shortcuts: Vec<Widget<Shortcut>>,
    divider: Widget<Label>,
}

impl Shortcuts {
    pub fn new(shortcuts: Vec<Widget<Shortcut>>, divider: Widget<Label>) -> Self {
        Self { shortcuts, divider }
    }
}

impl WidgetComponent for Shortcuts {
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

    fn perform(&mut self, _properties: &Props, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

pub struct ContextBar {
    label_0: Widget<Label>,
    label_1: Widget<Label>,
    label_2: Widget<Label>,
    label_3: Widget<Label>,
    label_4: Widget<Label>,
}

impl ContextBar {
    pub fn new(
        label_0: Widget<Label>,
        label_1: Widget<Label>,
        label_2: Widget<Label>,
        label_3: Widget<Label>,
        label_4: Widget<Label>,
    ) -> Self {
        Self {
            label_0,
            label_1,
            label_2,
            label_3,
            label_4,
        }
    }
}

impl WidgetComponent for ContextBar {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();

        let label_0_w = self.label_0.query(Attribute::Width).unwrap().unwrap_size();
        let label_1_w = self.label_1.query(Attribute::Width).unwrap().unwrap_size();
        let label_2_w = self.label_2.query(Attribute::Width).unwrap().unwrap_size();
        let label_4_w = self.label_4.query(Attribute::Width).unwrap().unwrap_size();

        if display {
            let layout = layout::h_stack(
                vec![
                    self.label_0.clone().to_boxed(),
                    self.label_1.clone().to_boxed(),
                    self.label_3
                        .clone()
                        .width(
                            area.width
                                .saturating_sub(label_0_w + label_1_w + label_2_w + label_4_w),
                        )
                        .to_boxed(),
                    self.label_2.clone().to_boxed(),
                    self.label_4.clone().to_boxed(),
                ],
                area,
            );

            for (mut component, area) in layout {
                component.view(frame, area);
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

pub fn bar(
    theme: &Theme,
    label_0: &str,
    label_1: &str,
    label_2: &str,
    label_3: &str,
    label_4: &str,
) -> Widget<ContextBar> {
    let context = super::label(&format!(" {label_0} ")).background(theme.colors.context_badge_bg);
    let id = super::label(&format!(" {label_1} "))
        .foreground(theme.colors.context_color_fg)
        .background(theme.colors.context_bg);
    let title = super::label(&format!(" {label_2} "))
        .foreground(theme.colors.default_fg)
        .background(theme.colors.context_bg);
    let author = super::label(&format!(" {label_3} "))
        .foreground(theme.colors.context_light)
        .background(theme.colors.context_bg);
    let comments = super::label(&format!(" {label_4} "))
        .foreground(theme.colors.context_light)
        .background(theme.colors.context_bg);

    let context_bar = ContextBar::new(context, id, author, title, comments);

    Widget::new(context_bar).height(1)
}
