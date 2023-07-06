use tuirealm::command::{Cmd, CmdResult};
use tuirealm::props::{Alignment, AttrValue, Attribute, Color, Props, Style};
use tuirealm::tui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::tui::text::{Span, Spans, Text};
use tuirealm::{Frame, MockComponent, State, StateValue};

use crate::ui::theme::Theme;
use crate::ui::widget::{Widget, WidgetComponent};

/// A label that can be styled using a foreground color and text modifiers.
/// Its height is fixed, its width depends on the length of the text it displays.
#[derive(Clone, Default)]
pub struct Label;

impl WidgetComponent for Label {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        use tui_realm_stdlib::Label;

        let content = properties
            .get_or(Attribute::Content, AttrValue::String(String::default()))
            .unwrap_string();
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
                    .text(content),
                None => Label::default()
                    .foreground(foreground)
                    .background(background)
                    .text(content),
            };

            label.view(frame, area);
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

impl From<&Widget<Label>> for Span<'_> {
    fn from(label: &Widget<Label>) -> Self {
        let content = label
            .query(Attribute::Content)
            .unwrap_or(AttrValue::String(String::default()))
            .unwrap_string();

        Span::styled(content, Style::default())
    }
}

impl From<&Widget<Label>> for Text<'_> {
    fn from(label: &Widget<Label>) -> Self {
        let content = label
            .query(Attribute::Content)
            .unwrap_or(AttrValue::String(String::default()))
            .unwrap_string();
        let foreground = label
            .query(Attribute::Foreground)
            .unwrap_or(AttrValue::Color(Color::Reset))
            .unwrap_color();

        Text::styled(content, Style::default().fg(foreground))
    }
}

pub struct Textarea {
    /// The current theme.
    theme: Theme,
    /// The scroll offset.
    offset: usize,
    /// The percentage scrolled.
    scroll_percent: usize,
}

impl Textarea {
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            offset: 0,
            scroll_percent: 0,
        }
    }

    fn scroll_percent(offset: usize, len: usize, height: usize) -> usize {
        if height >= len {
            100
        } else {
            let y = offset as f64;
            let h = height as f64;
            let t = len.saturating_sub(1) as f64;
            let v = y / (t - h) * 100_f64;

            std::cmp::max(0, std::cmp::min(100, v as usize))
        }
    }
}

impl WidgetComponent for Textarea {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        use tuirealm::tui::widgets::Paragraph;

        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();
        let fg = properties
            .get_or(Attribute::Foreground, AttrValue::Color(Color::Reset))
            .unwrap_color();

        let content = properties
            .get_or(Attribute::Content, AttrValue::String(String::default()))
            .unwrap_string();

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);

        let highlight_color = if focus {
            self.theme.colors.container_border_focus_fg
        } else {
            self.theme.colors.container_border_fg
        };

        // TODO: replace with `ratatui`'s reflow module when that becomes
        // public: https://github.com/tui-rs-revival/ratatui/pull/9.
        //
        // In the future, there should be highlighting for e.g. Markdown which
        // needs be done before wrapping. So this should rather wrap styled text
        // spans than plain text.
        let body = textwrap::wrap(&content, area.width.saturating_sub(2) as usize);
        let len = body.len();
        let height = layout[0].height - 1;

        let body: String = body.iter().map(|line| format!("{}\n", line)).collect();

        let paragraph = Paragraph::new(body)
            .scroll((self.offset as u16, 0))
            .style(Style::default().fg(fg));
        frame.render_widget(paragraph, layout[0]);

        self.scroll_percent = Self::scroll_percent(self.offset, len, height as usize);

        let progress = Spans::from(vec![Span::styled(
            format!("{} %", self.scroll_percent),
            Style::default().fg(highlight_color),
        )]);

        let progress = Paragraph::new(progress).alignment(Alignment::Right);
        frame.render_widget(progress, layout[1]);
    }

    fn state(&self) -> State {
        State::One(StateValue::Usize(self.offset))
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        use tuirealm::command::Direction;

        match cmd {
            Cmd::Scroll(Direction::Up) => {
                self.offset = self.offset.saturating_sub(1);
                CmdResult::None
            }
            Cmd::Scroll(Direction::Down) => {
                if self.scroll_percent < 100 {
                    self.offset = self.offset.saturating_add(1);
                }
                CmdResult::None
            }
            _ => CmdResult::None,
        }
    }
}
