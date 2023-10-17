use std::collections::LinkedList;

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::props::{BorderType, Borders, Style, TextModifiers};
use tuirealm::tui::layout::{Constraint, Direction, Margin, Rect};
use tuirealm::{AttrValue, Attribute, Frame, MockComponent, Props, State, StateValue};

use crate::ui::state::FormState;
use crate::ui::theme::Theme;
use crate::ui::widget::{Widget, WidgetComponent};

use super::container::Container;
use super::label::Label;

pub struct TextField {
    input: Widget<Container>,
    placeholder: Option<Widget<Label>>,
}

impl TextField {
    pub fn new(
        theme: Theme,
        input: Box<dyn MockComponent>,
        placeholder: Option<Widget<Label>>,
    ) -> Self {
        let container = crate::ui::container(&theme, input);

        Self {
            input: container,
            placeholder,
        }
    }
}

impl WidgetComponent for TextField {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        let show_placeholder = match self.input.state() {
            State::Vec(values) => match values.first() {
                Some(StateValue::String(input)) => values.len() == 1 && input.is_empty(),
                _ => false,
            },
            _ => false,
        };

        self.input.attr(Attribute::Focus, AttrValue::Flag(focus));
        self.input.view(frame, area);

        if show_placeholder {
            let inner = area.inner(&Margin {
                vertical: 1,
                horizontal: 2,
            });
            if let Some(placeholder) = &mut self.placeholder {
                placeholder.view(frame, inner);
            }
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
        self.input.perform(cmd)
    }
}

pub struct TextArea {
    input: Widget<Container>,
    placeholder: Option<Widget<Label>>,
}

impl TextArea {
    pub fn new(
        theme: Theme,
        input: Box<dyn MockComponent>,
        placeholder: Option<Widget<Label>>,
    ) -> Self {
        let container = crate::ui::container(&theme, input);

        Self {
            input: container,
            placeholder,
        }
    }
}

impl WidgetComponent for TextArea {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        let show_placeholder = match self.input.state() {
            State::Vec(values) => match values.first() {
                Some(StateValue::String(input)) => values.len() == 1 && input.is_empty(),
                _ => false,
            },
            _ => false,
        };

        self.input.attr(Attribute::Focus, AttrValue::Flag(focus));
        self.input.view(frame, area);

        if show_placeholder {
            let inner = area.inner(&Margin {
                vertical: 1,
                horizontal: 2,
            });
            if let Some(placeholder) = &mut self.placeholder {
                placeholder.view(frame, inner);
            }
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
            Cmd::Custom(Form::CMD_ENTER) => Cmd::Custom(TEXTAREA_CMD_NEWLINE),
            _ => cmd,
        };
        self.input.perform(cmd)
    }
}

pub struct Radio {
    input: tui_realm_stdlib::Radio,
}

impl Radio {
    pub fn new(theme: Theme, _title: &str, choices: &[String], selected: u16) -> Self {
        let input = tui_realm_stdlib::Radio::default()
            .borders(
                Borders::default()
                    .modifiers(BorderType::Rounded)
                    .color(theme.colors.container_border_focus_fg),
            )
            .inactive(
                Style::default()
                    .fg(theme.colors.container_border_fg)
                    .add_modifier(TextModifiers::REVERSED),
            )
            .foreground(theme.colors.default_fg)
            .choices(choices)
            .value(selected as usize);

        Self { input }
    }
}

impl WidgetComponent for Radio {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        self.input.attr(Attribute::Focus, AttrValue::Flag(focus));
        self.input.view(frame, area);
    }

    fn state(&self) -> State {
        self.input.state()
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        let cmd = match cmd {
            Cmd::Custom(Form::CMD_ENTER) => Cmd::Submit,
            _ => cmd,
        };
        self.input.perform(cmd)
    }
}

pub struct Form {
    // This form's fields: title, tags, assignees, description.
    fields: Vec<Box<dyn MockComponent>>,
    // This form's hidden fields.
    hidden: Vec<Box<dyn MockComponent>>,
    /// State that holds the current focus etc.
    state: FormState,
}

impl Form {
    pub const CMD_FOCUS_PREVIOUS: &str = "cmd-focus-previous";
    pub const CMD_FOCUS_NEXT: &str = "cmd-focus-next";
    pub const CMD_ENTER: &str = "cmd-enter";
    pub const CMD_PASTE: &str = "cmd-paste";

    pub fn new(
        _theme: Theme,
        fields: Vec<Box<dyn MockComponent>>,
        hidden: Vec<Box<dyn MockComponent>>,
    ) -> Self {
        let state = FormState::new(Some(0), fields.len());

        Self {
            fields,
            hidden,
            state,
        }
    }
}

impl WidgetComponent for Form {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        use tuirealm::props::Layout;
        // Clear and set current focus
        let focus = self.state.focus().unwrap_or(0);
        for field in &mut self.fields {
            field.attr(Attribute::Focus, AttrValue::Flag(false));
        }
        if let Some(field) = self.fields.get_mut(focus) {
            field.attr(Attribute::Focus, AttrValue::Flag(true));
        }

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                &self
                    .fields
                    .iter()
                    .map(|_| Constraint::Length(3))
                    .collect::<Vec<_>>(),
            );
        let layout = properties
            .get_or(Attribute::Layout, AttrValue::Layout(layout))
            .unwrap_layout();
        let layout = layout.chunks(area);

        for (index, area) in layout.iter().enumerate().take(self.fields.len()) {
            if let Some(field) = self.fields.get_mut(index) {
                field.view(frame, *area);
            }
        }
    }

    fn state(&self) -> State {
        let fields = self
            .hidden
            .iter()
            .chain(self.fields.iter())
            .collect::<Vec<_>>();
        let states = fields
            .iter()
            .map(|field| field.state())
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
                if let Some(field) = self.fields.get_mut(focus) {
                    return field.perform(cmd);
                }
                CmdResult::None
            }
        }
    }
}

pub fn hidden_field(theme: &Theme, value: Option<&str>) -> Widget<TextField> {
    let input = tui_realm_textarea::TextArea::new(vec![value.unwrap_or_default().to_owned()]);

    Widget::new(TextField::new(theme.clone(), Box::new(input), None)).display(false)
}

pub fn text_field(
    theme: &Theme,
    _title: &str,
    placeholder: &str,
    value: Option<&str>,
) -> Widget<TextField> {
    let input = tui_realm_textarea::TextArea::new(vec![value.unwrap_or_default().to_owned()])
        .wrap(true)
        .single_line(true)
        .cursor_line_style(Style::reset())
        .style(Style::default().fg(theme.colors.default_fg));
    let placeholder = crate::ui::label(placeholder).foreground(theme.colors.input_placeholder_fg);

    Widget::new(TextField::new(
        theme.clone(),
        Box::new(input),
        Some(placeholder),
    ))
}

pub fn text_area(
    theme: &Theme,
    _title: &str,
    placeholder: &str,
    value: Option<&str>,
) -> Widget<TextArea> {
    let input = tui_realm_textarea::TextArea::new(vec![value.unwrap_or_default().to_owned()])
        .wrap(true)
        .single_line(false)
        .cursor_line_style(Style::reset())
        .style(Style::default().fg(theme.colors.default_fg));
    let placeholder = crate::ui::label(placeholder).foreground(theme.colors.input_placeholder_fg);

    Widget::new(TextArea::new(
        theme.clone(),
        Box::new(input),
        Some(placeholder),
    ))
}

pub fn radio(theme: &Theme, title: &str, choices: &[String], selected: u16) -> Widget<Radio> {
    Widget::new(Radio::new(theme.clone(), title, choices, selected))
}
