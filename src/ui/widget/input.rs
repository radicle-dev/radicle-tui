use std::marker::PhantomData;

use ratatui::Frame;
use termion::event::Key;

use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span, Text};

use super::{utils, RenderProps, View, ViewProps, ViewState};

#[derive(Clone)]
pub struct TextFieldProps {
    pub title: String,
    pub inline_label: bool,
    pub show_cursor: bool,
    pub text: String,
}

impl TextFieldProps {
    pub fn text(mut self, new_text: &str) -> Self {
        if self.text != new_text {
            self.text = String::from(new_text);
        }
        self
    }

    pub fn title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    pub fn inline(mut self, inline: bool) -> Self {
        self.inline_label = inline;
        self
    }
}

impl Default for TextFieldProps {
    fn default() -> Self {
        Self {
            title: String::new(),
            inline_label: false,
            show_cursor: true,
            text: String::new(),
        }
    }
}

#[derive(Clone)]
struct TextFieldState {
    pub text: Option<String>,
    pub cursor_position: usize,
}

pub struct TextField<S, M> {
    /// Internal state
    state: TextFieldState,
    /// Phantom
    phantom: PhantomData<(S, M)>,
}

impl<S, M> Default for TextField<S, M> {
    fn default() -> Self {
        Self {
            state: TextFieldState {
                text: None,
                cursor_position: 0,
            },
            phantom: PhantomData,
        }
    }
}

impl<S, M> TextField<S, M> {
    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.state.cursor_position.saturating_sub(1);
        self.state.cursor_position = self.clamp_cursor(cursor_moved_left);
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.state.cursor_position.saturating_add(1);
        self.state.cursor_position = self.clamp_cursor(cursor_moved_right);
    }

    fn enter_char(&mut self, new_char: char) {
        self.state.text = Some(self.state.text.clone().unwrap_or_default());
        self.state
            .text
            .as_mut()
            .unwrap()
            .insert(self.state.cursor_position, new_char);
        self.move_cursor_right();
    }

    fn delete_char_right(&mut self) {
        self.state.text = Some(self.state.text.clone().unwrap_or_default());

        // Method "remove" is not used on the saved text for deleting the selected char.
        // Reason: Using remove on String works on bytes instead of the chars.
        // Using remove would require special care because of char boundaries.

        let current_index = self.state.cursor_position;
        let from_left_to_current_index = current_index;

        // Getting all characters before the selected character.
        let before_char_to_delete = self
            .state
            .text
            .as_ref()
            .unwrap()
            .chars()
            .take(from_left_to_current_index);
        // Getting all characters after selected character.
        let after_char_to_delete = self
            .state
            .text
            .as_ref()
            .unwrap()
            .chars()
            .skip(current_index.saturating_add(1));

        // Put all characters together except the selected one.
        // By leaving the selected one out, it is forgotten and therefore deleted.
        self.state.text = Some(before_char_to_delete.chain(after_char_to_delete).collect());
    }

    fn delete_char_left(&mut self) {
        self.state.text = Some(self.state.text.clone().unwrap_or_default());

        let is_not_cursor_leftmost = self.state.cursor_position != 0;
        if is_not_cursor_leftmost {
            // Method "remove" is not used on the saved text for deleting the selected char.
            // Reason: Using remove on String works on bytes instead of the chars.
            // Using remove would require special care because of char boundaries.

            let current_index = self.state.cursor_position;
            let from_left_to_current_index = current_index - 1;

            // Getting all characters before the selected character.
            let before_char_to_delete = self
                .state
                .text
                .as_ref()
                .unwrap()
                .chars()
                .take(from_left_to_current_index);
            // Getting all characters after selected character.
            let after_char_to_delete = self
                .state
                .text
                .as_ref()
                .unwrap()
                .chars()
                .skip(current_index);

            // Put all characters together except the selected one.
            // By leaving the selected one out, it is forgotten and therefore deleted.
            self.state.text = Some(before_char_to_delete.chain(after_char_to_delete).collect());
            self.move_cursor_left();
        }
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.state.text.clone().unwrap_or_default().len())
    }
}

impl<S, M> View for TextField<S, M>
where
    S: 'static,
    M: 'static,
{
    type Message = M;
    type State = S;

    fn view_state(&self) -> Option<ViewState> {
        self.state
            .text
            .as_ref()
            .map(|text| ViewState::String(text.to_string()))
    }

    fn reset(&mut self) {
        self.state = TextFieldState {
            text: None,
            cursor_position: 0,
        };
    }

    fn handle_event(&mut self, _props: Option<&ViewProps>, key: Key) -> Option<Self::Message> {
        match key {
            Key::Char(to_insert)
                if (key != Key::Alt('\n'))
                    && (key != Key::Char('\n'))
                    && (key != Key::Ctrl('\n')) =>
            {
                self.enter_char(to_insert);
            }
            Key::Backspace => {
                self.delete_char_left();
            }
            Key::Delete => {
                self.delete_char_right();
            }
            Key::Left => {
                self.move_cursor_left();
            }
            Key::Right => {
                self.move_cursor_right();
            }
            _ => {}
        }

        None
    }

    fn update(&mut self, props: Option<&ViewProps>, _state: &Self::State) {
        let default = TextFieldProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<TextFieldProps>())
            .unwrap_or(&default);

        if self.state.text.is_none() {
            self.state.cursor_position = props.text.len().saturating_sub(1);
        }
        self.state.text = Some(props.text.clone());
    }

    fn render(&mut self, props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let default = TextFieldProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<TextFieldProps>())
            .unwrap_or(&default);

        let area = render.area;
        let layout = Layout::vertical(Constraint::from_lengths([1, 1])).split(area);

        let text = self.state.text.clone().unwrap_or_default();
        let input = text.as_str();
        let label = format!(" {} ", props.title);
        let overline = String::from("▔").repeat(area.width as usize);
        let cursor_pos = self.state.cursor_position as u16;

        if props.inline_label {
            let top_layout = Layout::horizontal([
                Constraint::Length(label.chars().count() as u16),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(layout[0]);

            let label = Span::from(label.clone()).magenta().dim().reversed();
            let input = Span::from(input).reset();

            let overline = Line::from([Span::raw(overline).magenta().dim()].to_vec());

            frame.render_widget(label, top_layout[0]);
            frame.render_widget(input, top_layout[2]);
            frame.render_widget(overline, layout[1]);

            if props.show_cursor {
                frame.set_cursor(top_layout[2].x + cursor_pos, top_layout[2].y)
            }
        } else {
            let top = Line::from([Span::from(input).reset()].to_vec());
            let bottom = Line::from(
                [
                    Span::from(label).magenta().dim().reversed(),
                    Span::raw(overline).magenta().dim(),
                ]
                .to_vec(),
            );

            frame.render_widget(top, layout[0]);
            frame.render_widget(bottom, layout[1]);

            if props.show_cursor {
                frame.set_cursor(area.x + cursor_pos, area.y)
            }
        }
    }
}

/// The state of a `TextArea`.
#[derive(Clone, Default, Debug)]
pub struct TextAreaState {
    /// Current vertical scroll position.
    pub scroll: usize,
    /// Current cursor position.
    pub cursor: (usize, usize),
}

/// The properties of a `TextArea`.
#[derive(Clone)]
pub struct TextAreaProps<'a> {
    /// Content of this text area.
    content: Text<'a>,
    /// Current cursor position. Default: `(0, 0)`.
    cursor: (usize, usize),
    /// If this text area should handle events. Default: `true`.
    handle_keys: bool,
    /// If this text area is in insert mode. Default: `false`.
    insert_mode: bool,
    /// If this text area should render its scroll progress. Default: `false`.
    show_scroll_progress: bool,
    /// If this text area should render its cursor progress. Default: `false`.
    show_column_progress: bool,
}

impl<'a> Default for TextAreaProps<'a> {
    fn default() -> Self {
        Self {
            content: String::new().into(),
            cursor: (0, 0),
            handle_keys: true,
            insert_mode: false,
            show_scroll_progress: false,
            show_column_progress: false,
        }
    }
}

impl<'a> TextAreaProps<'a> {
    pub fn content<T>(mut self, content: T) -> Self
    where
        T: Into<Text<'a>>,
    {
        self.content = content.into();
        self
    }

    pub fn cursor(mut self, cursor: (usize, usize)) -> Self {
        self.cursor = cursor;
        self
    }

    pub fn show_scroll_progress(mut self, show_scroll_progress: bool) -> Self {
        self.show_scroll_progress = show_scroll_progress;
        self
    }

    pub fn show_column_progress(mut self, show_column_progress: bool) -> Self {
        self.show_column_progress = show_column_progress;
        self
    }

    pub fn handle_keys(mut self, handle_keys: bool) -> Self {
        self.handle_keys = handle_keys;
        self
    }
}

/// A non-editable text area that can be behave like a text editor.
/// It can scroll through text by moving around the cursor.
pub struct TextArea<'a, S, M> {
    phantom: PhantomData<(S, M)>,
    textarea: tui_textarea::TextArea<'a>,
    area: (u16, u16),
}

impl<'a, S, M> Default for TextArea<'a, S, M> {
    fn default() -> Self {
        Self {
            phantom: PhantomData,
            textarea: tui_textarea::TextArea::default(),
            area: (0, 0),
        }
    }
}

impl<'a, S, M> View for TextArea<'a, S, M> {
    type State = S;
    type Message = M;

    fn handle_event(&mut self, props: Option<&ViewProps>, key: Key) -> Option<Self::Message> {
        use tui_textarea::Input;

        let default = TextAreaProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<TextAreaProps>())
            .unwrap_or(&default);

        if props.handle_keys {
            if !props.insert_mode {
                match key {
                    Key::Left | Key::Char('h') => {
                        self.textarea.input(Input {
                            key: tui_textarea::Key::Left,
                            ..Default::default()
                        });
                    }
                    Key::Right | Key::Char('l') => {
                        self.textarea.input(Input {
                            key: tui_textarea::Key::Right,
                            ..Default::default()
                        });
                    }
                    Key::Up | Key::Char('k') => {
                        self.textarea.input(Input {
                            key: tui_textarea::Key::Up,
                            ..Default::default()
                        });
                    }
                    Key::Down | Key::Char('j') => {
                        self.textarea.input(Input {
                            key: tui_textarea::Key::Down,
                            ..Default::default()
                        });
                    }
                    _ => {}
                }
            } else {
                // TODO: Implement insert mode.
            }
        }

        None
    }

    fn update(&mut self, props: Option<&ViewProps>, _state: &Self::State) {
        let default = TextAreaProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<TextAreaProps>())
            .unwrap_or(&default);

        self.textarea = tui_textarea::TextArea::new(
            props
                .content
                .lines
                .iter()
                .map(|line| line.to_string())
                .collect::<Vec<_>>(),
        );
    }

    fn render(&mut self, props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let default = TextAreaProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<TextAreaProps>())
            .unwrap_or(&default);

        let [area] = Layout::default()
            .constraints([Constraint::Min(1)])
            .horizontal_margin(1)
            .areas(render.area);

        let [content_area, progress_area] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(
                if props.show_scroll_progress || props.show_column_progress {
                    1
                } else {
                    0
                },
            ),
        ])
        .areas(area);

        let cursor_line_style = Style::default();
        let cursor_style = if render.focus {
            Style::default().reversed()
        } else {
            cursor_line_style
        };
        let content_style = if render.focus {
            Style::default()
        } else {
            Style::default().dim()
        };

        self.textarea.move_cursor(tui_textarea::CursorMove::Jump(
            props.cursor.0 as u16,
            props.cursor.1 as u16,
        ));
        self.textarea.set_cursor_line_style(cursor_line_style);
        self.textarea.set_cursor_style(cursor_style);
        self.textarea.set_style(content_style);

        let (scroll_progress, cursor_progress) = (
            utils::scroll::percent_absolute(
                self.textarea.cursor().0,
                props.content.lines.len(),
                content_area.height.into(),
            ),
            (self.textarea.cursor().0, self.textarea.cursor().1),
        );

        frame.render_widget(self.textarea.widget(), content_area);

        let mut progress_info = vec![];

        if props.show_scroll_progress {
            progress_info.push(Span::styled(
                format!("{}%", scroll_progress),
                Style::default().dim(),
            ))
        }

        if props.show_scroll_progress && props.show_column_progress {
            progress_info.push(Span::raw(" "));
        }

        if props.show_column_progress {
            progress_info.push(Span::styled(
                format!("[{},{}]", cursor_progress.0, cursor_progress.1),
                Style::default().dim(),
            ))
        }

        frame.render_widget(
            Line::from(progress_info).alignment(Alignment::Right),
            progress_area,
        );

        self.area = (content_area.height, content_area.width);
    }

    fn view_state(&self) -> Option<ViewState> {
        Some(ViewState::TextArea(TextAreaState {
            cursor: self.textarea.cursor(),
            scroll: utils::scroll::percent_absolute(
                self.textarea.cursor().0.saturating_sub(self.area.0.into()),
                self.textarea.lines().len(),
                self.area.0.into(),
            ),
        }))
    }
}

/// State of a `TextView`.
#[derive(Clone, Default, Debug)]
pub struct TextViewState {
    /// Current vertical scroll position.
    pub scroll: usize,
    /// Current cursor position.
    pub cursor: (usize, usize),
}

/// Properties of a `TextView`.
#[derive(Clone)]
pub struct TextViewProps<'a> {
    /// Content of this text view.
    content: Text<'a>,
    /// Current cursor position. Default: `(0, 0)`.
    cursor: (usize, usize),
    /// If this widget should handle events. Default: `true`.
    handle_keys: bool,
    /// If this widget should render its scroll progress. Default: `false`.
    show_scroll_progress: bool,
    /// An optional text that is rendered inside the footer bar on the bottom.
    footer: Option<Text<'a>>,
}

impl<'a> TextViewProps<'a> {
    pub fn content<T>(mut self, content: T) -> Self
    where
        T: Into<Text<'a>>,
    {
        self.content = content.into();
        self
    }

    pub fn footer<T>(mut self, footer: Option<T>) -> Self
    where
        T: Into<Text<'a>>,
    {
        self.footer = footer.map(|f| f.into());
        self
    }

    pub fn cursor(mut self, cursor: (usize, usize)) -> Self {
        self.cursor = cursor;
        self
    }

    pub fn show_scroll_progress(mut self, show_scroll_progress: bool) -> Self {
        self.show_scroll_progress = show_scroll_progress;
        self
    }

    pub fn handle_keys(mut self, handle_keys: bool) -> Self {
        self.handle_keys = handle_keys;
        self
    }
}

impl<'a> Default for TextViewProps<'a> {
    fn default() -> Self {
        Self {
            content: String::new().into(),
            cursor: (0, 0),
            handle_keys: true,
            show_scroll_progress: false,
            footer: None,
        }
    }
}

/// A scrollable, non-editable text view widget. It can scroll through text by
/// moving around the viewport.
pub struct TextView<S, M> {
    /// Internal view state.
    state: TextViewState,
    /// Current render area.
    area: (u16, u16),
    /// Phantom.
    phantom: PhantomData<(S, M)>,
}

impl<S, M> Default for TextView<S, M> {
    fn default() -> Self {
        Self {
            state: TextViewState {
                scroll: 0,
                cursor: (0, 0),
            },
            area: (0, 0),
            phantom: PhantomData,
        }
    }
}

impl<S, M> TextView<S, M> {
    fn scroll_up(&mut self) {
        self.state.cursor.0 = self.state.cursor.0.saturating_sub(1);
    }

    fn scroll_down(&mut self, len: usize, page_size: usize) {
        let end = len.saturating_sub(page_size);
        self.state.cursor.0 = std::cmp::min(self.state.cursor.0.saturating_add(1), end);
    }

    fn scroll_left(&mut self) {
        self.state.cursor.1 = self.state.cursor.1.saturating_sub(3);
    }

    fn scroll_right(&mut self, max_line_length: usize) {
        self.state.cursor.1 = std::cmp::min(
            self.state.cursor.1.saturating_add(3),
            max_line_length.saturating_add(3),
        );
    }

    fn prev_page(&mut self, page_size: usize) {
        self.state.cursor.0 = self.state.cursor.0.saturating_sub(page_size);
    }

    fn next_page(&mut self, len: usize, page_size: usize) {
        let end = len.saturating_sub(page_size);

        self.state.cursor.0 = std::cmp::min(self.state.cursor.0.saturating_add(page_size), end);
    }

    fn begin(&mut self) {
        self.state.cursor.0 = 0;
    }

    fn end(&mut self, len: usize, page_size: usize) {
        self.state.cursor.0 = len.saturating_sub(page_size);
    }
}

impl<S, M> View for TextView<S, M>
where
    S: 'static,
    M: 'static,
{
    type Message = M;
    type State = S;

    fn handle_event(&mut self, props: Option<&ViewProps>, key: Key) -> Option<Self::Message> {
        let default = TextViewProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<TextViewProps>())
            .unwrap_or(&default);

        let len = props.content.lines.len();
        let max_line_len = props
            .content
            .lines
            .iter()
            .map(|l| l.width())
            .max()
            .unwrap_or_default();
        let page_size = self.area.0 as usize;

        if props.handle_keys {
            match key {
                Key::Up | Key::Char('k') => {
                    self.scroll_up();
                }
                Key::Down | Key::Char('j') => {
                    self.scroll_down(len, page_size);
                }
                Key::Left | Key::Char('h') => {
                    self.scroll_left();
                }
                Key::Right | Key::Char('l') => {
                    self.scroll_right(max_line_len.saturating_sub(self.area.1.into()));
                }
                Key::PageUp => {
                    self.prev_page(page_size);
                }
                Key::PageDown => {
                    self.next_page(len, page_size);
                }
                Key::Home => {
                    self.begin();
                }
                Key::End => {
                    self.end(len, page_size);
                }
                _ => {}
            }
        }

        self.state.scroll = utils::scroll::percent_absolute(
            self.state.cursor.0,
            props.content.lines.len(),
            self.area.0.into(),
        );

        None
    }

    fn update(&mut self, props: Option<&ViewProps>, _state: &Self::State) {
        let default = TextViewProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<TextViewProps>())
            .unwrap_or(&default);

        if props.cursor != self.state.cursor {
            self.state.cursor = props.cursor;
        }
    }

    fn render(&mut self, props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let default = TextViewProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<TextViewProps>())
            .unwrap_or(&default);

        let [area] = Layout::default()
            .constraints([Constraint::Min(1)])
            .horizontal_margin(1)
            .areas(render.area);

        let render_footer = props.show_scroll_progress || props.footer.is_some();
        let [content_area, footer_area] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(if render_footer { 1 } else { 0 }),
        ])
        .areas(area);

        let style = if render.focus {
            Style::default()
        } else {
            Style::default().dim()
        };

        let content = ratatui::widgets::Paragraph::new(props.content.clone())
            .style(style)
            .scroll((self.state.cursor.0 as u16, self.state.cursor.1 as u16));

        let scroll_progress = utils::scroll::percent_absolute(
            self.state.cursor.0,
            props.content.lines.len(),
            content_area.height.into(),
        );

        let progress_info = if props.show_scroll_progress {
            vec![Span::styled(
                format!("{}%", scroll_progress),
                Style::default().dim(),
            )]
        } else {
            vec![]
        };

        frame.render_widget(content, content_area);
        frame.render_widget(
            props
                .footer
                .as_ref()
                .cloned()
                .unwrap_or_default()
                .alignment(Alignment::Left)
                .dim(),
            footer_area,
        );
        frame.render_widget(
            Line::from(progress_info).alignment(Alignment::Right),
            footer_area,
        );

        self.area = (content_area.height, content_area.width);
    }

    fn view_state(&self) -> Option<ViewState> {
        Some(ViewState::TextView(self.state.clone()))
    }
}
