use termion::event::Key;

use tokio::sync::mpsc::UnboundedSender;

use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::{Backend, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};

use super::{Render, View, Widget};

pub struct TextFieldProps {
    title: String,
    inline_label: bool,
    show_cursor: bool,
    text: String,
    cursor_position: usize,
}

impl Default for TextFieldProps {
    fn default() -> Self {
        Self {
            title: String::new(),
            inline_label: false,
            show_cursor: true,
            text: String::new(),
            cursor_position: 0,
        }
    }
}

pub struct TextField<A> {
    /// Message sender
    pub action_tx: UnboundedSender<A>,
    /// Internal props
    props: TextFieldProps,
}

impl<A> TextField<A> {
    pub fn read(&self) -> &str {
        &self.props.text
    }

    pub fn text(mut self, new_text: &str) -> Self {
        if self.props.text != new_text {
            self.props.text = String::from(new_text);
            self.props.cursor_position = self.props.text.len();
        }
        self
    }

    pub fn title(mut self, title: &str) -> Self {
        self.props.title = title.to_string();
        self
    }

    pub fn inline(mut self, inline: bool) -> Self {
        self.props.inline_label = inline;
        self
    }

    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.props.cursor_position.saturating_sub(1);
        self.props.cursor_position = self.clamp_cursor(cursor_moved_left);
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.props.cursor_position.saturating_add(1);
        self.props.cursor_position = self.clamp_cursor(cursor_moved_right);
    }

    fn enter_char(&mut self, new_char: char) {
        self.props.text.insert(self.props.cursor_position, new_char);
        self.move_cursor_right();
    }

    fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.props.cursor_position != 0;
        if is_not_cursor_leftmost {
            // Method "remove" is not used on the saved text for deleting the selected char.
            // Reason: Using remove on String works on bytes instead of the chars.
            // Using remove would require special care because of char boundaries.

            let current_index = self.props.cursor_position;
            let from_left_to_current_index = current_index - 1;

            // Getting all characters before the selected character.
            let before_char_to_delete = self.props.text.chars().take(from_left_to_current_index);
            // Getting all characters after selected character.
            let after_char_to_delete = self.props.text.chars().skip(current_index);

            // Put all characters together except the selected one.
            // By leaving the selected one out, it is forgotten and therefore deleted.
            self.props.text = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.props.text.len())
    }
}

impl<S, A> View<S, A> for TextField<A> {
    fn new(state: &S, action_tx: UnboundedSender<A>) -> Self {
        Self {
            action_tx,
            props: TextFieldProps::default(),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, _state: &S) -> Self {
        self
    }

    fn update(&mut self, state: &S) {}

    fn handle_key_event(&mut self, key: Key) {
        match key {
            Key::Char(to_insert)
                if (key != Key::Alt('\n'))
                    && (key != Key::Char('\n'))
                    && (key != Key::Ctrl('\n')) =>
            {
                self.enter_char(to_insert);
            }
            Key::Backspace => {
                self.delete_char();
            }
            Key::Left => {
                self.move_cursor_left();
            }
            Key::Right => {
                self.move_cursor_right();
            }
            _ => {}
        }
    }
}

impl<A, B: Backend> Render<B, ()> for TextField<A> {
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let layout = Layout::vertical(Constraint::from_lengths([1, 1])).split(area);

        let input = self.props.text.as_str();
        let label = format!(" {} ", self.props.title);
        let overline = String::from("▔").repeat(area.width as usize);
        let cursor_pos = self.props.cursor_position as u16;

        if self.props.inline_label {
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

            if self.props.show_cursor {
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

            if self.props.show_cursor {
                frame.set_cursor(area.x + cursor_pos, area.y)
            }
        }
    }
}

impl<S, A: 'static, B: Backend> Widget<S, A, B> for TextField<A> {}
