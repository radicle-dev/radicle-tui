use tui_realm_textarea::TextArea;

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::props::Style;
use tuirealm::tui::layout::{Constraint, Direction, Margin, Rect};
use tuirealm::tui::text::Text;
use tuirealm::tui::widgets::Paragraph;
use tuirealm::{AttrValue, Attribute, Frame, MockComponent, Props, State, StateValue};

use crate::ui::state::FormState;
use crate::ui::theme::Theme;
use crate::ui::widget::{Widget, WidgetComponent};

use super::container::Container;
use super::label::{Label, Textarea};

pub struct TextInput {
    lines: Vec<String>,
    cursor: (usize, usize), // 0-base
    theme: Theme,
    /// The scroll offset.
    offset: usize,
    /// The current line count.
    len: usize,
    /// The current display height.
    height: usize,
}

impl TextInput {
    pub const PROP_MULTILINE: &str = "prop-multiline";
    pub const CMD_NEWLINE: &str = "cmd-newline";
    pub const CMD_PASTE: &str = "cmd-paste";

    pub fn new(theme: Theme) -> Self {
        Self {
            lines: vec![String::new()],
            cursor: (0, 0),
            offset: 0,
            len: 0,
            height: 0,
            theme,
        }
    }

    pub fn insert_char(&mut self, c: char) {
        let (row, col) = self.cursor;
        let line = &mut self.lines[row];
        let i = line
            .char_indices()
            .nth(col)
            .map(|(i, _)| i)
            .unwrap_or(line.len());
        line.insert(i, c);
        self.cursor.1 += 1;
    }
}

impl WidgetComponent for TextInput {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        /////////////////////////////////////////
        // let mut textarea = Widget::new(Textarea::new(self.theme.clone()))
        //     .foreground(self.theme.colors.default_fg)
        //     .content(AttrValue::String(self.lines[0].clone()));

        // textarea.attr(Attribute::Focus, AttrValue::Flag(focus));
        // textarea.view(frame, area);
        /////////////////////////////////////////

        let content: String = self
            .lines
            .iter()
            .map(|line| format!("{}\n", line))
            .collect();

        let body = textwrap::wrap(&content, area.width.saturating_sub(2) as usize);
        self.len = body.len();
        self.height = (area.height - 1) as usize;

        let body: String = body.iter().map(|line| format!("{}\n", line)).collect();

        let paragraph = Paragraph::new(body)
            .scroll((self.offset as u16, 0))
            .style(Style::default().fg(self.theme.colors.default_fg));
        frame.render_widget(paragraph, area);

        if focus {
            frame.set_cursor(area.x.saturating_add(self.cursor.1 as u16), area.y);
        }
    }

    fn state(&self) -> State {
        State::Vec(
            self.lines
                .iter()
                .map(|line| StateValue::String(line.to_string()))
                .collect(),
        )
    }

    fn perform(&mut self, properties: &Props, cmd: Cmd) -> CmdResult {
        match cmd {
            Cmd::Type('\t') => {
                // self.widget.insert_tab();
                CmdResult::None
            }
            Cmd::Type('\n') | Cmd::Custom(Field::CMD_NEWLINE) => {
                // self.widget.insert_newline();
                CmdResult::None
            }
            Cmd::Type(ch) => {
                self.insert_char(ch);
                CmdResult::None
            }
            _ => CmdResult::None,
        }
    }
}

pub struct Field {
    input: Widget<Container>,
    placeholder: Widget<Label>,
    show_placeholder: bool,
}

impl Field {
    pub const PROP_MULTILINE: &str = TextInput::PROP_MULTILINE;
    pub const CMD_NEWLINE: &str = TextInput::CMD_NEWLINE;
    pub const CMD_PASTE: &str = TextInput::CMD_PASTE;

    pub fn new(theme: Theme, title: &str) -> Self {
        // let input = TextArea::default()
        //     .cursor_line_style(Style::reset())
        //     .style(Style::default().fg(theme.colors.default_fg));
        let input = Widget::new(TextInput::new(theme.clone()));
        let container = super::container(&theme, Box::new(input));

        Self {
            input: container,
            placeholder: super::label(title).foreground(theme.colors.input_placeholder_fg),
            show_placeholder: true,
        }
    }
}

impl WidgetComponent for Field {
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
                    self.show_placeholder = values.len() == 1 && input.is_empty();
                } else {
                    self.show_placeholder = false;
                }
            }
            result
        }
    }
}

pub struct Form {
    // This form's fields: title, tags, assignees, description.
    inputs: Vec<Widget<Field>>,
    /// State that holds the current focus etc.
    state: FormState,
}

impl Form {
    pub const CMD_FOCUS_PREVIOUS: &str = "cmd-focus-previous";
    pub const CMD_FOCUS_NEXT: &str = "cmd-focus-next";

    pub fn new(_theme: Theme, inputs: Vec<Widget<Field>>) -> Self {
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
        match cmd {
            Cmd::Custom(Self::CMD_FOCUS_PREVIOUS) => {
                self.state.focus_previous();
                CmdResult::None
            }
            Cmd::Custom(Self::CMD_FOCUS_NEXT) => {
                self.state.focus_next();
                CmdResult::None
            }
            Cmd::Submit => {
                // Fold each input's vector of lines into a single string
                // that contains newlines and return a state vector with
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
