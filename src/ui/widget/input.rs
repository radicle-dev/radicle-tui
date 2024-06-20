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

/// Configuration of a `TextArea`'s internal progress display.
#[derive(Default, Clone)]
pub struct TextAreaProgressInfo {
    scroll: bool,
    cursor: bool,
}

impl TextAreaProgressInfo {
    pub fn scroll(mut self, scroll: bool) -> Self {
        self.scroll = scroll;
        self
    }

    pub fn cursor(mut self, cursor: bool) -> Self {
        self.cursor = cursor;
        self
    }

    pub fn is_rendered(&self) -> bool {
        self.scroll || self.cursor
    }
}

#[derive(Clone)]
pub struct TextAreaProps<'a> {
    content: Text<'a>,
    cursor: (usize, usize),
    can_scroll: bool,
    progress_info: TextAreaProgressInfo,
}

impl<'a> Default for TextAreaProps<'a> {
    fn default() -> Self {
        Self {
            content: String::new().into(),
            cursor: (0, 0),
            can_scroll: true,
            progress_info: TextAreaProgressInfo::default(),
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

    pub fn progress_info(mut self, progress_info: TextAreaProgressInfo) -> Self {
        self.progress_info = progress_info;
        self
    }

    pub fn can_scroll(mut self, can_scroll: bool) -> Self {
        self.can_scroll = can_scroll;
        self
    }
}

pub struct TextArea<'a, S, M> {
    phantom: PhantomData<(S, M)>,
    textarea: tui_textarea::TextArea<'a>,
    height: u16,
}

impl<'a, S, M> Default for TextArea<'a, S, M> {
    fn default() -> Self {
        Self {
            phantom: PhantomData,
            textarea: tui_textarea::TextArea::default(),
            height: 0,
        }
    }
}

impl<'a, S, M> View for TextArea<'a, S, M> {
    type State = S;
    type Message = M;

    fn handle_event(&mut self, props: Option<&ViewProps>, key: Key) -> Option<Self::Message> {
        let default = TextAreaProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<TextAreaProps>())
            .unwrap_or(&default);

        if props.can_scroll {
            match key {
                Key::Left => {
                    self.textarea.input(tui_textarea::Input {
                        key: tui_textarea::Key::Left,
                        ..Default::default()
                    });
                }
                Key::Right => {
                    self.textarea.input(tui_textarea::Input {
                        key: tui_textarea::Key::Right,
                        ..Default::default()
                    });
                }
                Key::Up => {
                    self.textarea.input(tui_textarea::Input {
                        key: tui_textarea::Key::Up,
                        ..Default::default()
                    });
                }
                Key::Down => {
                    self.textarea.input(tui_textarea::Input {
                        key: tui_textarea::Key::Down,
                        ..Default::default()
                    });
                }
                _ => {}
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

        self.height = render.area.height;

        self.textarea.move_cursor(tui_textarea::CursorMove::Jump(
            props.cursor.0 as u16,
            props.cursor.1 as u16,
        ));
        self.textarea.set_cursor_line_style(cursor_line_style);
        self.textarea.set_cursor_style(cursor_style);
        self.textarea.set_style(content_style);

        if props.progress_info.is_rendered() {
            let [content_area, progress_area] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);

            let mut progress_info = vec![];

            if props.progress_info.scroll {
                progress_info.push(Span::styled(
                    format!(
                        "{}%",
                        utils::scroll::percent_absolute(
                            self.textarea.cursor().0,
                            self.textarea.lines().len(),
                            content_area.height.into()
                        )
                    ),
                    Style::default().dim(),
                ))
            }

            if props.progress_info.scroll && props.progress_info.cursor {
                progress_info.push(Span::raw(" "));
            }

            if props.progress_info.cursor {
                progress_info.push(Span::styled(
                    format!(
                        "[{},{}]",
                        self.textarea.cursor().0,
                        self.textarea.cursor().1
                    ),
                    Style::default().dim(),
                ))
            }

            let line = Line::from(progress_info).alignment(Alignment::Right);

            frame.render_widget(self.textarea.widget(), content_area);
            frame.render_widget(line, progress_area);
        } else {
            frame.render_widget(self.textarea.widget(), area);
        }
    }

    fn view_state(&self) -> Option<ViewState> {
        Some(ViewState::TextArea {
            scroll: utils::scroll::percent_absolute(
                self.textarea.cursor().0.saturating_sub(self.height.into()),
                self.textarea.lines().len(),
                self.height.into(),
            ),
            cursor: self.textarea.cursor(),
        })
    }
}
