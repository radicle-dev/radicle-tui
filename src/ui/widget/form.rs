use std::collections::LinkedList;

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::props::Style;
use tuirealm::tui::layout::{Constraint, Direction, Margin, Rect};
use tuirealm::{AttrValue, Attribute, Frame, MockComponent, Props, State, StateValue};

use crate::ui::state::FormState;
use crate::ui::theme::{style, Theme};
use crate::ui::widget::{Widget, WidgetComponent};

use super::container::Container;
use super::label::{self, Label};

pub struct TextField {
    input: Widget<Container>,
    placeholder: Widget<Label>,
    show_placeholder: bool,
}

impl TextField {
    pub fn new(theme: Theme, title: &str) -> Self {
        let input = tui_realm_textarea::TextArea::default()
            .wrap(false)
            .single_line(true)
            .cursor_line_style(Style::reset())
            .style(style::reset());
        let container = crate::ui::container(&theme, Box::new(input));

        Self {
            input: container,
            placeholder: label::default(title).style(style::gray_dim()),
            show_placeholder: true,
        }
    }
}

impl WidgetComponent for TextField {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        self.input.attr(Attribute::Focus, AttrValue::Flag(focus));
        self.input.view(frame, area);

        if self.show_placeholder {
            let inner = area.inner(&Margin {
                vertical: 1,
                horizontal: 2,
            });
            self.placeholder.view(frame, inner);
        }
    }

    fn state(&self) -> State {
        if let State::Vec(values) = self.input.state() {
            let text = match values.get(0) {
                Some(StateValue::String(line)) => line.clone(),
                _ => String::new(),
            };

            State::One(StateValue::String(text))
        } else {
            State::None
        }
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        use tui_realm_textarea::*;

        let cmd = match cmd {
            Cmd::Custom(Form::CMD_PASTE) => Cmd::Custom(TEXTAREA_CMD_PASTE),
            _ => cmd,
        };
        let result = self.input.perform(cmd);

        if let State::Vec(values) = self.input.state() {
            if let Some(StateValue::String(input)) = values.first() {
                self.show_placeholder = values.len() == 1 && input.is_empty();
            } else {
                self.show_placeholder = false;
            }
        }
        result
    }
}

pub struct TextArea {
    input: Widget<Container>,
    placeholder: Widget<Label>,
    show_placeholder: bool,
}

impl TextArea {
    pub fn new(theme: Theme, title: &str) -> Self {
        let input = tui_realm_textarea::TextArea::default()
            .wrap(true)
            .single_line(false)
            .cursor_line_style(Style::reset())
            .style(style::reset());
        let container = crate::ui::container(&theme, Box::new(input));

        Self {
            input: container,
            placeholder: label::default(title).style(style::gray_dim()),
            show_placeholder: true,
        }
    }
}

impl WidgetComponent for TextArea {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        self.input.attr(Attribute::Focus, AttrValue::Flag(focus));
        self.input.view(frame, area);

        if self.show_placeholder {
            let inner = area.inner(&Margin {
                vertical: 1,
                horizontal: 2,
            });
            self.placeholder.view(frame, inner);
        }
    }

    fn state(&self) -> State {
        // Fold each input's vector of lines into a single string.
        if let State::Vec(values) = self.input.state() {
            let mut text = String::new();
            let lines = values
                .iter()
                .map(|value| match value {
                    StateValue::String(line) => line.clone(),
                    _ => String::new(),
                })
                .collect::<Vec<_>>();

            let mut lines = lines.iter().peekable();
            while let Some(line) = lines.next() {
                text.push_str(line);
                if lines.peek().is_some() {
                    text.push('\n');
                }
            }

            State::One(StateValue::String(text))
        } else {
            State::None
        }
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        use tui_realm_textarea::*;

        let cmd = match cmd {
            Cmd::Custom(Form::CMD_PASTE) => Cmd::Custom(TEXTAREA_CMD_PASTE),
            Cmd::Custom(Form::CMD_NEWLINE) => Cmd::Custom(TEXTAREA_CMD_NEWLINE),
            _ => cmd,
        };
        let result = self.input.perform(cmd);

        if let State::Vec(values) = self.input.state() {
            if let Some(StateValue::String(input)) = values.first() {
                self.show_placeholder = values.len() == 1 && input.is_empty();
            } else {
                self.show_placeholder = false;
            }
        }
        result
    }
}

pub struct Form {
    // This form's fields: title, tags, assignees, description.
    inputs: Vec<Box<dyn MockComponent>>,
    /// State that holds the current focus etc.
    state: FormState,
}

impl Form {
    pub const CMD_FOCUS_PREVIOUS: &'static str = "cmd-focus-previous";
    pub const CMD_FOCUS_NEXT: &'static str = "cmd-focus-next";
    pub const CMD_NEWLINE: &'static str = "cmd-newline";
    pub const CMD_PASTE: &'static str = "cmd-paste";

    pub const PROP_ID: &'static str = "prop-id";

    pub fn new(_theme: Theme, inputs: Vec<Box<dyn MockComponent>>) -> Self {
        let state = FormState::new(Some(0), inputs.len());

        Self { inputs, state }
    }
}

impl WidgetComponent for Form {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        use tuirealm::props::Layout;
        // Clear and set current focus
        let focus = self.state.focus().unwrap_or(0);
        for input in &mut self.inputs {
            input.attr(Attribute::Focus, AttrValue::Flag(false));
        }
        if let Some(input) = self.inputs.get_mut(focus) {
            input.attr(Attribute::Focus, AttrValue::Flag(true));
        }

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                &self
                    .inputs
                    .iter()
                    .map(|_| Constraint::Length(3))
                    .collect::<Vec<_>>(),
            );
        let layout = properties
            .get_or(Attribute::Layout, AttrValue::Layout(layout))
            .unwrap_layout();
        let layout = layout.chunks(area);

        for (index, area) in layout.iter().enumerate().take(self.inputs.len()) {
            if let Some(input) = self.inputs.get_mut(index) {
                input.view(frame, *area);
            }
        }
    }

    fn state(&self) -> State {
        let states = self
            .inputs
            .iter()
            .map(|input| input.state())
            .collect::<LinkedList<_>>();
        State::Linked(states)
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        match cmd {
            Cmd::Custom(Self::CMD_FOCUS_PREVIOUS) => {
                self.state.focus_previous();
                CmdResult::None
            }
            Cmd::Custom(Self::CMD_FOCUS_NEXT) => {
                self.state.focus_next();
                CmdResult::None
            }
            Cmd::Submit => CmdResult::Submit(self.state()),
            _ => {
                let focus = self.state.focus().unwrap_or(0);
                if let Some(input) = self.inputs.get_mut(focus) {
                    return input.perform(cmd);
                }
                CmdResult::None
            }
        }
    }
}
