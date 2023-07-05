use tui_realm_textarea::TextArea;

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::props::Style;
use tuirealm::tui::layout::{Constraint, Direction, Margin, Rect};
use tuirealm::{AttrValue, Attribute, Frame, MockComponent, Props, State, StateValue};

use crate::ui::state::FormState;
use crate::ui::theme::Theme;
use crate::ui::widget::{Widget, WidgetComponent};

use super::container::Container;
use super::label::Label;

pub struct TextInput {
    input: Widget<Container>,
    placeholder: Widget<Label>,
    show_placeholder: bool,
}

impl TextInput {
    pub const PROP_MULTILINE: &str = "multiline";
    pub const CMD_NEWLINE: &str = tui_realm_textarea::TEXTAREA_CMD_NEWLINE;

    pub fn new(theme: Theme, title: &str) -> Self {
        let input = TextArea::default()
            .cursor_line_style(Style::reset())
            .style(Style::default().fg(theme.colors.default_fg));
        let container = super::container(&theme, Box::new(input));

        Self {
            input: container,
            placeholder: super::label(title).foreground(theme.colors.input_placeholder_fg),
            show_placeholder: true,
        }
    }
}

impl WidgetComponent for TextInput {
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
        self.input.state()
    }

    fn perform(&mut self, properties: &Props, cmd: Cmd) -> CmdResult {
        let multiline = properties
            .get_or(
                Attribute::Custom(Self::PROP_MULTILINE),
                AttrValue::Flag(false),
            )
            .unwrap_flag();

        if !multiline && cmd == Cmd::Custom(Self::CMD_NEWLINE) {
            CmdResult::None
        } else {
            let result = self.input.perform(cmd);
            if let State::Vec(values) = self.input.state() {
                if let Some(StateValue::String(input)) = values.first() {
                    self.show_placeholder = input.is_empty();
                }
            }
            result
        }
    }
}

pub struct Form {
    // This form's fields: title, tags, assignees, description.
    inputs: Vec<Widget<TextInput>>,
    /// State that holds the current focus etc.
    state: FormState,
}

impl Form {
    pub fn new(_theme: Theme, inputs: Vec<Widget<TextInput>>) -> Self {
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
        State::None
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        use tuirealm::command::Direction;
        match cmd {
            Cmd::Move(Direction::Up) => {
                self.state.focus_previous();
                CmdResult::None
            }
            Cmd::Move(Direction::Down) => {
                self.state.focus_next();
                CmdResult::None
            }
            Cmd::Submit => {
                // Fold each input's vector of lines into a single string
                // that containes newlines and return a state vector with
                // each entry being the folded input string.
                let states = self
                    .inputs
                    .iter()
                    .map(|input| {
                        if let State::Vec(values) = input.state() {
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

                            StateValue::String(text)
                        } else {
                            StateValue::None
                        }
                    })
                    .collect::<Vec<_>>();
                CmdResult::Submit(State::Vec(states))
            }
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
