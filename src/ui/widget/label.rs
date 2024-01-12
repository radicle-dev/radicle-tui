use tuirealm::command::{Cmd, CmdResult};
use tuirealm::props::{Alignment, AttrValue, Attribute, Color, Props, Style};
use tuirealm::tui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::tui::text::{Span, Spans, Text};
use tuirealm::{Frame, MockComponent, State, StateValue};

use crate::ui::layout;
use crate::ui::theme::style;
use crate::ui::widget::{Widget, WidgetComponent};

pub fn default(content: &str) -> Widget<Label> {
    // TODO: Remove when size constraints are implemented
    let width = content.chars().count() as u16;

    Widget::new(Label)
        .content(AttrValue::String(content.to_string()))
        .height(1)
        .width(width)
}

pub fn group(labels: &[Widget<Label>]) -> Widget<LabelGroup> {
    let group = LabelGroup::new(labels);
    let width = labels.iter().fold(0, |total, label| {
        total
            + label
                .query(Attribute::Width)
                .unwrap_or(AttrValue::Size(0))
                .unwrap_size()
    });

    Widget::new(group).width(width)
}

pub fn reversable(content: &str) -> Widget<Label> {
    let content = &format!(" {content} ");

    default(content)
}

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
        let style = properties
            .get_or(Attribute::Style, AttrValue::Style(Style::default()))
            .unwrap_style();

        if display {
            let mut label = Label::default()
                .foreground(style.fg.unwrap_or(Color::Reset))
                .background(style.bg.unwrap_or(Color::Reset))
                .modifiers(style.add_modifier)
                .text(content);

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

#[derive(Clone, Default)]
pub struct LabelGroup {
    labels: Vec<Widget<Label>>,
}

impl LabelGroup {
    pub fn new(labels: &[Widget<Label>]) -> Self {
        Self {
            labels: labels.to_vec(),
        }
    }
}

impl WidgetComponent for LabelGroup {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();

        if display {
            let mut labels: Vec<Box<dyn MockComponent>> = vec![];
            for label in &self.labels {
                labels.push(label.clone().to_boxed());
            }

            let layout = layout::h_stack(labels, area);
            for (mut label, area) in layout {
                label.view(frame, area);
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

#[derive(Default)]
pub struct Textarea {
    /// The scroll offset.
    offset: usize,
    /// The current line count.
    len: usize,
    /// The current display height.
    height: usize,
    /// The percentage scrolled.
    scroll_percent: usize,
}

impl Textarea {
    pub const PROP_DISPLAY_PROGRESS: &'static str = "display-progress";

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
        let display_progress = properties
            .get_or(
                Attribute::Custom(Self::PROP_DISPLAY_PROGRESS),
                AttrValue::Flag(false),
            )
            .unwrap_flag();

        let content = properties
            .get_or(Attribute::Content, AttrValue::String(String::default()))
            .unwrap_string();

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);

        // TODO: replace with `ratatui`'s reflow module when that becomes
        // public: https://github.com/tui-rs-revival/ratatui/pull/9.
        //
        // In the future, there should be highlighting for e.g. Markdown which
        // needs be done before wrapping. So this should rather wrap styled text
        // spans than plain text.
        let body = textwrap::wrap(&content, area.width.saturating_sub(2) as usize);
        self.len = body.len();
        self.height = (layout[0].height - 1) as usize;

        let body: String = body.iter().fold(String::new(), |mut body, line| {
            body.push_str(&format!("{}\n", line));
            body
        });

        let paragraph = Paragraph::new(body)
            .scroll((self.offset as u16, 0))
            .style(style::reset());
        frame.render_widget(paragraph, layout[0]);

        self.scroll_percent = Self::scroll_percent(self.offset, self.len, self.height);

        if display_progress {
            let progress = Spans::from(vec![Span::styled(
                format!("{} %", self.scroll_percent),
                style::border(focus),
            )]);

            let progress = Paragraph::new(progress).alignment(Alignment::Right);
            frame.render_widget(progress, layout[1]);
        }
    }

    fn state(&self) -> State {
        State::One(StateValue::Usize(self.scroll_percent))
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        use tuirealm::command::Direction;

        match cmd {
            Cmd::Scroll(Direction::Up) => {
                self.offset = self.offset.saturating_sub(1);
                self.scroll_percent = Self::scroll_percent(self.offset, self.len, self.height);
                CmdResult::None
            }
            Cmd::Scroll(Direction::Down) => {
                if self.scroll_percent < 100 {
                    self.offset = self.offset.saturating_add(1);
                    self.scroll_percent = Self::scroll_percent(self.offset, self.len, self.height);
                }
                CmdResult::None
            }
            _ => CmdResult::None,
        }
    }
}
