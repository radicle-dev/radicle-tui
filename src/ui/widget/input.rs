use std::any::Any;

use termion::event::Key;

use tokio::sync::mpsc::UnboundedSender;

use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::{Backend, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};

use super::{EventCallback, Properties, UpdateCallback, View, Widget};

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

impl Properties for TextFieldProps {}
pub struct TextFieldState {
    pub text: String,
    pub cursor_position: usize,
}

pub struct TextField<S, A> {
    /// Internal props
    props: TextFieldProps,
    /// Message sender
    action_tx: UnboundedSender<A>,
    /// Custom update handler
    on_update: Option<UpdateCallback<S>>,
    /// Additional custom event handler
    on_change: Option<EventCallback<A>>,
    /// Internal state
    state: TextFieldState,
}

impl<S, A> TextField<S, A> {
    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.state.cursor_position.saturating_sub(1);
        self.state.cursor_position = self.clamp_cursor(cursor_moved_left);
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.state.cursor_position.saturating_add(1);
        self.state.cursor_position = self.clamp_cursor(cursor_moved_right);
    }

    fn enter_char(&mut self, new_char: char) {
        self.state.text.insert(self.state.cursor_position, new_char);
        self.move_cursor_right();
    }

    fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.state.cursor_position != 0;
        if is_not_cursor_leftmost {
            // Method "remove" is not used on the saved text for deleting the selected char.
            // Reason: Using remove on String works on bytes instead of the chars.
            // Using remove would require special care because of char boundaries.

            let current_index = self.state.cursor_position;
            let from_left_to_current_index = current_index - 1;

            // Getting all characters before the selected character.
            let before_char_to_delete = self.state.text.chars().take(from_left_to_current_index);
            // Getting all characters after selected character.
            let after_char_to_delete = self.state.text.chars().skip(current_index);

            // Put all characters together except the selected one.
            // By leaving the selected one out, it is forgotten and therefore deleted.
            self.state.text = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.state.text.len())
    }
}

impl<S, A> View<S, A> for TextField<S, A> {
    fn new(_state: &S, action_tx: UnboundedSender<A>) -> Self {
        Self {
            action_tx,
            props: TextFieldProps::default(),
            on_update: None,
            on_change: None,
            state: TextFieldState {
                text: String::new(),
                cursor_position: 0,
            },
        }
    }

    fn on_update(mut self, callback: UpdateCallback<S>) -> Self {
        self.on_update = Some(callback);
        self
    }

    fn on_change(mut self, callback: EventCallback<A>) -> Self {
        self.on_change = Some(callback);
        self
    }

    fn update(&mut self, state: &S) {
        if let Some(on_update) = self.on_update {
            if let Some(props) = (on_update)(state).downcast_ref::<TextFieldProps>() {
                self.props = props.clone();
                self.state.text = props.text.clone();
                self.state.cursor_position = props.text.len().saturating_sub(1);
            }
        }
    }

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

        if let Some(on_change) = self.on_change {
            (on_change)(&self.state, self.action_tx.clone());
        }
    }
}

impl<B, S, A> Widget<B, S, A> for TextField<S, A>
where
    B: Backend,
{
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, props: &dyn Any) {
        let props = props
            .downcast_ref::<TextFieldProps>()
            .unwrap_or(&self.props);

        let layout = Layout::vertical(Constraint::from_lengths([1, 1])).split(area);

        let input = self.state.text.as_str();
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
