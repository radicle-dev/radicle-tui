use tuirealm::command::{Cmd, CmdResult};
use tuirealm::props::{AttrValue, Attribute, Props};
use tuirealm::tui::layout::Rect;
use tuirealm::{Frame, MockComponent, State};

use super::label::{Label, LabelGroup};

use crate::ui::layout;
use crate::ui::theme::{style, Theme};
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
    col_0: Widget<LabelGroup>,
    col_1: Widget<LabelGroup>,
    col_2: Widget<LabelGroup>,
    col_3: Widget<LabelGroup>,
    col_4: Widget<LabelGroup>,
}

impl ContextBar {
    pub const PROP_EDIT_MODE: &'static str = "edit-mode";

    pub fn new(
        col_0: Widget<LabelGroup>,
        col_1: Widget<LabelGroup>,
        col_2: Widget<LabelGroup>,
        col_3: Widget<LabelGroup>,
        col_4: Widget<LabelGroup>,
    ) -> Self {
        Self {
            col_0,
            col_1,
            col_2,
            col_3,
            col_4,
        }
    }
}

impl WidgetComponent for ContextBar {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();
        let edit_mode = properties
            .get_or(
                Attribute::Custom(Self::PROP_EDIT_MODE),
                AttrValue::Flag(false),
            )
            .unwrap_flag();

        let col_0_w = self.col_0.query(Attribute::Width).unwrap().unwrap_size();
        let col_1_w = self.col_1.query(Attribute::Width).unwrap().unwrap_size();
        let col_3_w = self.col_3.query(Attribute::Width).unwrap().unwrap_size();
        let col_4_w = self.col_4.query(Attribute::Width).unwrap().unwrap_size();

        if edit_mode {
            self.col_0.attr(
                Attribute::Background,
                AttrValue::Color(style::yellow_reversed().bg.unwrap()),
            )
        }

        if display {
            let layout = layout::h_stack(
                vec![
                    self.col_0.clone().to_boxed(),
                    self.col_1.clone().to_boxed(),
                    self.col_2
                        .clone()
                        .width(
                            area.width
                                .saturating_sub(col_0_w + col_1_w + col_3_w + col_4_w),
                        )
                        .to_boxed(),
                    self.col_3.clone().to_boxed(),
                    self.col_4.clone().to_boxed(),
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
    _theme: &Theme,
    label_0: &str,
    label_1: &str,
    label_2: &str,
    label_3: &str,
    label_4: &str,
) -> Widget<ContextBar> {
    use crate::ui::{label, label_group};

    let label_0 = label(&format!(" {label_0} ")).style(style::magenta_reversed());
    let label_1 = label(&format!(" {label_1} ")).style(style::default_reversed());
    let label_2 = label(&format!(" {label_2} ")).style(style::default_reversed());
    let label_3 = label(&format!(" {label_3} ")).style(style::default_reversed());
    let label_4 = label(&format!(" {label_4} ")).style(style::default_reversed());

    let label_0 = label_group(&[label_0]);
    let label_1 = label_group(&[label_1]);
    let label_2 = label_group(&[label_2]);
    let label_3 = label_group(&[label_3]);
    let label_4 = label_group(&[label_4]);

    let context_bar = ContextBar::new(label_0, label_1, label_2, label_3, label_4);

    Widget::new(context_bar).height(1)
}
