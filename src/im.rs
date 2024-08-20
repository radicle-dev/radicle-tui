use std::collections::VecDeque;
use std::fmt::Debug;
use std::time::Duration;

use anyhow::Result;

use ratatui::layout::{Layout, Rect};
use tokio::sync::broadcast;
use tokio::sync::mpsc::UnboundedReceiver;

use termion::event::Key;

use ratatui::Frame;

use crate::event::Event;
use crate::store;
use crate::store::State;
use crate::task;
use crate::task::Interrupted;
use crate::terminal;
use crate::ui::theme::Theme;
use crate::Channel;

const RENDERING_TICK_RATE: Duration = Duration::from_millis(250);
const INLINE_HEIGHT: usize = 20;

pub trait App {
    type State;
    type Message;

    fn render(&self, ui: &mut Ui, frame: &mut Frame, state: &Self::State) -> Result<()>;
}

pub async fn run_app<S, M, P>(
    channel: Channel<M>,
    state: S,
    app: impl App<State = S, Message = M>,
) -> Result<Option<P>>
where
    S: State<P, Message = M> + Clone + Debug + Send + Sync + 'static,
    M: 'static,
    P: Clone + Debug + Send + Sync + 'static,
{
    let (terminator, mut interrupt_rx) = task::create_termination();

    let (store, state_rx) = store::Store::<S, M, P>::new();
    let frontend = Frontend::default();

    tokio::try_join!(
        store.main_loop(state, terminator, channel.rx, interrupt_rx.resubscribe()),
        frontend.im_main_loop(app, state_rx, interrupt_rx.resubscribe()),
    )?;

    if let Ok(reason) = interrupt_rx.recv().await {
        match reason {
            Interrupted::User { payload } => Ok(payload),
            Interrupted::OsSignal => anyhow::bail!("exited because of an os sig int"),
        }
    } else {
        anyhow::bail!("exited because of an unexpected error");
    }
}

#[derive(Default)]
pub struct Frontend {}

impl Frontend {
    pub async fn im_main_loop<S, M, P>(
        self,
        app: impl App<State = S, Message = M>,
        mut state_rx: UnboundedReceiver<S>,
        mut interrupt_rx: broadcast::Receiver<Interrupted<P>>,
    ) -> anyhow::Result<Interrupted<P>>
    where
        S: State<P> + 'static,
        M: 'static,
        P: Clone + Send + Sync + Debug,
    {
        let mut ticker = tokio::time::interval(RENDERING_TICK_RATE);

        let mut terminal = terminal::setup(INLINE_HEIGHT)?;
        let mut events_rx = terminal::events();

        let mut state = state_rx.recv().await.unwrap();
        let mut ui = Ui::default();

        let result: anyhow::Result<Interrupted<P>> = loop {
            tokio::select! {
                // Tick to terminate the select every N milliseconds
                _ = ticker.tick() => (),
                Some(event) = events_rx.recv() => match event {
                    Event::Key(key) => ui.store_input(key),
                    Event::Resize => (),
                },
                // Handle state updates
                Some(s) = state_rx.recv() => {
                    state = s;
                },
                // Catch and handle interrupt signal to gracefully shutdown
                Ok(interrupted) = interrupt_rx.recv() => {
                    let size = terminal.get_frame().size();
                    let _ = terminal.set_cursor(size.x, size.y);

                    break Ok(interrupted);
                }
            }
            terminal.draw(|frame| {
                let mut ui = ui.clone().with_area(frame.size());
                if let Err(err) = app.render(&mut ui, frame, &state) {
                    log::warn!("Drawing failed: {}", err);
                }
            })?;

            ui.clear_inputs();
        };

        terminal::restore(&mut terminal)?;

        result
    }
}

#[derive(Default, Debug)]
pub struct Response {
    pub changed: bool,
}

#[derive(Debug)]
pub struct InnerResponse<R> {
    /// What the user closure returned.
    pub inner: R,
    /// The response of the area.
    pub response: Response,
}

impl<R> InnerResponse<R> {
    #[inline]
    pub fn new(inner: R, response: Response) -> Self {
        Self { inner, response }
    }
}

pub trait Widget {
    fn ui(self, ui: &mut Ui, frame: &mut Frame) -> Response;
}

#[derive(Default, Clone, Debug)]
pub struct Ui {
    pub(crate) inputs: VecDeque<Key>,
    pub(crate) theme: Theme,
    pub(crate) area: Rect,
    pub(crate) layout: Layout,
    next_area: usize,
}

impl Ui {
    pub fn input(&mut self, f: impl Fn(Key) -> bool) -> bool {
        self.inputs.iter().find(|key| f(**key)).is_some()
    }

    pub fn input_with_key(&mut self, f: impl Fn(Key) -> bool) -> Option<Key> {
        self.inputs.iter().find(|key| f(**key)).copied()
    }

    pub fn store_input(&mut self, key: Key) {
        self.inputs.push_back(key);
    }

    pub fn clear_inputs(&mut self) {
        self.inputs.clear();
    }
}

impl Ui {
    pub fn new(area: Rect) -> Self {
        Self {
            area,
            ..Default::default()
        }
    }

    pub fn with_area(mut self, area: Rect) -> Self {
        self.area = area;
        self
    }

    pub fn with_layout(mut self, layout: Layout) -> Self {
        self.layout = layout;
        self
    }

    pub fn with_inputs(mut self, inputs: VecDeque<Key>) -> Self {
        self.inputs = inputs;
        self
    }

    pub fn area(&self) -> Rect {
        self.area
    }

    pub fn next_area(&mut self) -> Option<Rect> {
        let rect = self.layout.split(self.area).get(self.next_area).cloned();
        self.next_area = self.next_area + 1;
        rect
    }
}

impl Ui {
    pub fn add(&mut self, frame: &mut Frame, widget: impl Widget) -> Response {
        widget.ui(self, frame)
    }

    pub fn child_ui(&mut self, area: Rect, layout: Layout) -> Self {
        Ui::default()
            .with_area(area)
            .with_layout(layout)
            .with_inputs(self.inputs.clone())
    }

    pub fn build_layout<R>(
        &mut self,
        layout: Layout,
        add_contents: impl FnOnce(&mut Self) -> R,
    ) -> InnerResponse<R> {
        self.build_layout_dyn(layout, Box::new(add_contents))
    }

    pub fn build_layout_dyn<'a, R>(
        &mut self,
        layout: Layout,
        add_contents: Box<dyn FnOnce(&mut Self) -> R + 'a>,
    ) -> InnerResponse<R> {
        let mut child_ui = self.child_ui(self.area(), layout);
        let inner = add_contents(&mut child_ui);

        InnerResponse::new(inner, Response::default())
    }
}

impl Ui {
    pub fn shortcuts(
        &mut self,
        frame: &mut Frame,
        shortcuts: &[(String, String)],
        divider: char,
    ) -> Response {
        widget::Shortcuts::new(shortcuts, divider).ui(self, frame)
    }

    pub fn text_view(&mut self, frame: &mut Frame, text: String) -> Response {
        widget::TextView::new(text).ui(self, frame)
    }

    pub fn text_edit_singleline(
        &mut self,
        frame: &mut Frame,
        text: &mut String,
        cursor: &mut usize,
    ) -> Response {
        widget::TextEdit::new(text, cursor).ui(self, frame)
    }

    pub fn text_edit_labeled_singleline(
        &mut self,
        frame: &mut Frame,
        text: &mut String,
        cursor: &mut usize,
        label: impl ToString,
    ) -> Response {
        widget::TextEdit::new(text, cursor)
            .with_label(label)
            .ui(self, frame)
    }
}

pub mod widget {
    use ratatui::layout::Layout;
    use ratatui::style::Stylize;
    use ratatui::text::{Line, Span, Text};
    use ratatui::widgets::Row;
    use ratatui::Frame;
    use ratatui::{layout::Constraint, widgets::Paragraph};
    use termion::event::Key;

    use crate::ui::theme::style;

    use super::{Response, Widget, Ui};

    pub struct TextView {
        text: String,
    }

    impl TextView {
        pub fn new(text: impl ToString) -> Self {
            Self {
                text: text.to_string(),
            }
        }
    }

    impl Widget for TextView {
        fn ui(self, ui: &mut Ui, frame: &mut Frame) -> Response {
            let area = ui.next_area().unwrap_or_default();

            frame.render_widget(Paragraph::new(self.text), area);

            Response::default()
        }
    }

    #[derive(Clone, Debug)]
    pub struct TextEditState {
        pub text: String,
        pub cursor: usize,
    }

    impl TextEditState {
        fn move_cursor_left(&mut self) {
            let cursor_moved_left = self.cursor.saturating_sub(1);
            self.cursor = self.clamp_cursor(cursor_moved_left);
        }

        fn move_cursor_right(&mut self) {
            let cursor_moved_right = self.cursor.saturating_add(1);
            self.cursor = self.clamp_cursor(cursor_moved_right);
        }

        fn enter_char(&mut self, new_char: char) {
            self.text = self.text.clone();
            self.text.insert(self.cursor, new_char);
            self.move_cursor_right();
        }

        fn delete_char_right(&mut self) {
            self.text = self.text.clone();

            // Method "remove" is not used on the saved text for deleting the selected char.
            // Reason: Using remove on String works on bytes instead of the chars.
            // Using remove would require special care because of char boundaries.

            let current_index = self.cursor;
            let from_left_to_current_index = current_index;

            // Getting all characters before the selected character.
            let before_char_to_delete = self.text.chars().take(from_left_to_current_index);
            // Getting all characters after selected character.
            let after_char_to_delete = self.text.chars().skip(current_index.saturating_add(1));

            // Put all characters together except the selected one.
            // By leaving the selected one out, it is forgotten and therefore deleted.
            self.text = before_char_to_delete.chain(after_char_to_delete).collect();
        }

        fn delete_char_left(&mut self) {
            self.text = self.text.clone();

            let is_not_cursor_leftmost = self.cursor != 0;
            if is_not_cursor_leftmost {
                // Method "remove" is not used on the saved text for deleting the selected char.
                // Reason: Using remove on String works on bytes instead of the chars.
                // Using remove would require special care because of char boundaries.

                let current_index = self.cursor;
                let from_left_to_current_index = current_index - 1;

                // Getting all characters before the selected character.
                let before_char_to_delete = self.text.chars().take(from_left_to_current_index);
                // Getting all characters after selected character.
                let after_char_to_delete = self.text.chars().skip(current_index);

                // Put all characters together except the selected one.
                // By leaving the selected one out, it is forgotten and therefore deleted.
                self.text = before_char_to_delete.chain(after_char_to_delete).collect();
                self.move_cursor_left();
            }
        }

        fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
            new_cursor_pos.clamp(0, self.text.clone().len())
        }
    }

    pub struct TextEditOutput {
        pub response: Response,
        pub state: TextEditState,
    }

    pub struct TextEdit<'a> {
        text: &'a mut String,
        cursor: &'a mut usize,
        label: Option<String>,
        inline_label: bool,
        show_cursor: bool,
        dim: bool,
    }

    impl<'a> TextEdit<'a> {
        /// # Example
        /// 
        /// ```
        /// let mut state = TextEditState::default();
        /// let output = im::widget::TextEdit::new(&mut text, &mut cursor).show(ui, frame);
        /// if output.response.changed {
        ///     state = output.state;
        /// }
        /// ```
        pub fn new(text: &'a mut String, cursor: &'a mut usize) -> Self {
            Self {
                text,
                cursor,
                label: None,
                inline_label: true,
                show_cursor: true,
                dim: true,
            }
        }

        pub fn with_label(mut self, label: impl ToString) -> Self {
            self.label = Some(label.to_string());
            self
        }
    }

    impl<'a> TextEdit<'a> {
        pub fn show(self, ui: &mut Ui, frame: &mut Frame) -> TextEditOutput {
            let mut response = Response::default();

            let area = ui.next_area().unwrap_or_default();
            let layout = Layout::vertical(Constraint::from_lengths([1, 1])).split(area);

            let mut state = TextEditState {
                text: self.text.clone(),
                cursor: *self.cursor,
            };

            // let focus = !render.focus;
            let focus = true;

            // let input = self.text.as_str();
            let label_content = format!(" {} ", self.label.unwrap_or_default());
            let overline = String::from("▔").repeat(area.width as usize);
            let cursor_pos = *self.cursor as u16;

            if let Some(key) = ui.input_with_key(|_| true) {
                match key {
                    Key::Char(to_insert)
                        if (key != Key::Alt('\n'))
                            && (key != Key::Char('\n'))
                            && (key != Key::Ctrl('\n')) =>
                    {
                        state.enter_char(to_insert);
                    }
                    Key::Backspace => {
                        state.delete_char_left();
                    }
                    Key::Delete => {
                        state.delete_char_right();
                    }
                    Key::Left => {
                        state.move_cursor_left();
                    }
                    Key::Right => {
                        state.move_cursor_right();
                    }
                    _ => {}
                }
                response.changed = true;
            }

            let (label, input, overline) = if !focus && self.dim {
                (
                    Span::from(label_content.clone()).magenta().dim().reversed(),
                    Span::from(state.text.clone()).reset().dim(),
                    Span::raw(overline).magenta().dim(),
                )
            } else {
                (
                    Span::from(label_content.clone()).magenta().reversed(),
                    Span::from(state.text.clone()).reset(),
                    Span::raw(overline).magenta(),
                )
            };

            if self.inline_label {
                let top_layout = Layout::horizontal([
                    Constraint::Length(label_content.chars().count() as u16),
                    Constraint::Length(1),
                    Constraint::Min(1),
                ])
                .split(layout[0]);

                let overline = Line::from([overline].to_vec());

                frame.render_widget(label, top_layout[0]);
                frame.render_widget(input, top_layout[2]);
                frame.render_widget(overline, layout[1]);

                if self.show_cursor {
                    frame.set_cursor(top_layout[2].x + cursor_pos, top_layout[2].y)
                }
            } else {
                let top = Line::from([input].to_vec());
                let bottom = Line::from([label, overline].to_vec());

                frame.render_widget(top, layout[0]);
                frame.render_widget(bottom, layout[1]);

                if self.show_cursor {
                    frame.set_cursor(area.x + cursor_pos, area.y)
                }
            }

            *self.text = state.text.clone();
            *self.cursor = state.cursor;

            TextEditOutput { response, state }
        }
    }

    impl<'a> Widget for TextEdit<'a> {
        fn ui(self, ui: &mut Ui, frame: &mut Frame) -> Response {
            self.show(ui, frame).response
        }
    }

    pub struct Shortcuts {
        pub shortcuts: Vec<(String, String)>,
        pub divider: char,
    }

    impl Shortcuts {
        pub fn new(shortcuts: &[(String, String)], divider: char) -> Self {
            Self {
                shortcuts: shortcuts.to_vec(),
                divider,
            }
        }
    }

    impl Widget for Shortcuts {
        fn ui(self, ui: &mut Ui, frame: &mut Frame) -> Response {
            use ratatui::widgets::Table;

            let mut shortcuts = self.shortcuts.iter().peekable();
            let mut row = vec![];

            while let Some(shortcut) = shortcuts.next() {
                let short = Text::from(shortcut.0.clone()).style(ui.theme.shortcuts_keys_style);
                let long = Text::from(shortcut.1.clone()).style(ui.theme.shortcuts_action_style);
                let spacer = Text::from(String::new());
                let divider = Text::from(format!(" {} ", self.divider)).style(style::gray().dim());

                row.push((shortcut.0.chars().count(), short));
                row.push((1, spacer));
                row.push((shortcut.1.chars().count(), long));

                if shortcuts.peek().is_some() {
                    row.push((3, divider));
                }
            }

            let row_copy = row.clone();
            let row: Vec<Text<'_>> = row_copy
                .clone()
                .iter()
                .map(|(_, text)| text.clone())
                .collect();
            let widths: Vec<Constraint> = row_copy
                .clone()
                .iter()
                .map(|(width, _)| Constraint::Length(*width as u16))
                .collect();
            let table = Table::new([Row::new(row)], widths).column_spacing(0);

            let area = ui.next_area().unwrap_or_default();
            frame.render_widget(table, area);

            Response::default()
        }
    }
}
