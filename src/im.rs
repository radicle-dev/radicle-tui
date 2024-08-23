use std::collections::VecDeque;
use std::fmt::Debug;
use std::rc::Rc;
use std::time::Duration;

use anyhow::Result;

use tokio::sync::broadcast;
use tokio::sync::mpsc::UnboundedReceiver;

use termion::event::Key;

use ratatui::layout::{Constraint, Rect};
use ratatui::Frame;

use crate::event::Event;
use crate::store;
use crate::store::State;
use crate::task;
use crate::task::Interrupted;
use crate::terminal;
use crate::ui::theme::Theme;
use crate::ui::widget::container::Column;
use crate::ui::widget::list::ToRow;
use crate::Channel;

use self::widget::Widget;

const RENDERING_TICK_RATE: Duration = Duration::from_millis(250);
const INLINE_HEIGHT: usize = 20;

pub trait App {
    type State;
    type Message;

    fn update(&self, ui: &Context, frame: &mut Frame, state: &Self::State) -> Result<()>;
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
        let mut ctx = Context::default();

        let result: anyhow::Result<Interrupted<P>> = loop {
            tokio::select! {
                // Tick to terminate the select every N milliseconds
                _ = ticker.tick() => (),
                Some(event) = events_rx.recv() => match event {
                    Event::Key(key) => ctx.store_input(key),
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
                let ctx = ctx.clone().with_frame_size(frame.size());

                if let Err(err) = app.update(&ctx, frame, &state) {
                    log::warn!("Drawing failed: {}", err);
                }
            })?;

            ctx.clear_inputs();
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

#[derive(Clone, Default, Debug)]
pub struct Context {
    pub(crate) inputs: VecDeque<Key>,
    frame_size: Rect,
}

impl Context {
    pub fn new(frame_size: Rect) -> Self {
        Self {
            inputs: VecDeque::default(),
            frame_size,
        }
    }

    pub fn with_inputs(mut self, inputs: VecDeque<Key>) -> Self {
        self.inputs = inputs;
        self
    }

    pub fn with_frame_size(mut self, frame_size: Rect) -> Self {
        self.frame_size = frame_size;
        self
    }

    pub fn frame_size(&self) -> Rect {
        self.frame_size.clone()
    }

    pub fn store_input(&mut self, key: Key) {
        self.inputs.push_back(key);
    }

    pub fn clear_inputs(&mut self) {
        self.inputs.clear();
    }
}

pub enum Borders {
    All,
    Top,
    Sides,
    Bottom,
    BottomSides,
}

#[derive(Clone, Default, Debug)]
pub enum Layout {
    #[default]
    None,
    Wrapped {
        internal: ratatui::layout::Layout,
    },
    Expandable3 {
        left_only: bool,
    },
}

impl From<ratatui::layout::Layout> for Layout {
    fn from(layout: ratatui::layout::Layout) -> Self {
        Layout::Wrapped { internal: layout }
    }
}

impl Layout {
    pub fn split(&self, area: Rect) -> Rc<[Rect]> {
        match self {
            Layout::None => Rc::new([]),
            Layout::Wrapped { internal } => internal.split(area),
            Layout::Expandable3 { left_only } => {
                use ratatui::layout::Layout;

                if *left_only {
                    [area].into()
                } else if area.width <= 140 {
                    let [left, right] = Layout::horizontal([
                        Constraint::Percentage(50),
                        Constraint::Percentage(50),
                    ])
                    .areas(area);
                    let [right_top, right_bottom] =
                        Layout::vertical([Constraint::Percentage(65), Constraint::Percentage(35)])
                            .areas(right);

                    [left, right_top, right_bottom].into()
                } else {
                    Layout::horizontal([
                        Constraint::Percentage(33),
                        Constraint::Percentage(33),
                        Constraint::Percentage(33),
                    ])
                    .split(area)
                }
            }
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct Ui {
    pub(crate) theme: Theme,
    pub(crate) area: Rect,
    pub(crate) layout: Layout,
    next_area: usize,
    ctx: Context,
}

impl Ui {
    pub fn input(&mut self, f: impl Fn(Key) -> bool) -> bool {
        self.ctx.inputs.iter().find(|key| f(**key)).is_some()
    }

    pub fn input_with_key(&mut self, f: impl Fn(Key) -> bool) -> Option<Key> {
        self.ctx.inputs.iter().find(|key| f(**key)).copied()
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

    pub fn with_ctx(mut self, ctx: Context) -> Self {
        self.ctx = ctx;
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

    pub fn child_ui(&mut self, area: Rect, layout: impl Into<Layout>) -> Self {
        Ui::default()
            .with_area(area)
            .with_layout(layout.into())
            .with_ctx(self.ctx.clone())
    }

    pub fn layout<R>(
        &mut self,
        layout: impl Into<Layout>,
        add_contents: impl FnOnce(&mut Self) -> R,
    ) -> InnerResponse<R> {
        self.layout_dyn(layout, Box::new(add_contents))
    }

    pub fn layout_dyn<'a, R>(
        &mut self,
        layout: impl Into<Layout>,
        add_contents: Box<dyn FnOnce(&mut Self) -> R + 'a>,
    ) -> InnerResponse<R> {
        let area = self.next_area().unwrap_or_default();
        let mut child_ui = self.child_ui(area, layout);
        let inner = add_contents(&mut child_ui);

        InnerResponse::new(inner, Response::default())
    }
}

impl Ui {
    pub fn table<'a, R, const W: usize>(
        &mut self,
        frame: &mut Frame,
        selected: &mut Option<usize>,
        items: &'a Vec<R>,
        columns: Vec<Column<'a>>,
        borders: Option<Borders>,
    ) -> Response
    where
        R: ToRow<W> + Clone,
    {
        widget::Table::new(selected, items, columns, borders).ui(self, frame)
    }

    pub fn shortcuts(
        &mut self,
        frame: &mut Frame,
        shortcuts: &[(String, String)],
        divider: char,
    ) -> Response {
        widget::Shortcuts::new(shortcuts, divider).ui(self, frame)
    }

    pub fn columns<'a>(
        &mut self,
        frame: &mut Frame,
        columns: Vec<Column<'a>>,
        borders: Option<Borders>,
    ) -> Response {
        widget::Columns::new(columns, borders).ui(self, frame)
    }

    pub fn text_view(
        &mut self,
        frame: &mut Frame,
        text: String,
        borders: Option<Borders>,
    ) -> Response {
        widget::TextView::new(text, borders).ui(self, frame)
    }

    pub fn text_edit_singleline(
        &mut self,
        frame: &mut Frame,
        text: &mut String,
        cursor: &mut usize,
        borders: Option<Borders>,
    ) -> Response {
        widget::TextEdit::new(text, cursor, borders).ui(self, frame)
    }

    pub fn text_edit_labeled_singleline(
        &mut self,
        frame: &mut Frame,
        text: &mut String,
        cursor: &mut usize,
        label: impl ToString,
        border: Option<Borders>,
    ) -> Response {
        widget::TextEdit::new(text, cursor, border)
            .with_label(label)
            .ui(self, frame)
    }
}

pub mod widget {
    use std::cmp;

    use ratatui::layout::{Direction, Layout, Rect};
    use ratatui::style::{Style, Stylize};
    use ratatui::text::{Line, Span, Text};
    use ratatui::widgets::{Block, BorderType, Row, Scrollbar, ScrollbarState};
    use ratatui::Frame;
    use ratatui::{layout::Constraint, widgets::Paragraph};
    use termion::event::Key;

    use crate::ui::ext::{FooterBlock, FooterBlockType, HeaderBlock};
    use crate::ui::theme::style;
    use crate::ui::widget::container::Column;
    use crate::ui::widget::list::ToRow;
    use crate::ui::{layout, span};

    use super::{Borders, Context, InnerResponse, Response, Ui};

    pub trait Widget {
        fn ui(self, ui: &mut Ui, frame: &mut Frame) -> Response;
    }

    #[derive(Default)]
    pub struct Window {}

    impl Window {
        #[inline]
        pub fn show<R>(
            self,
            ctx: &Context,
            add_contents: impl FnOnce(&mut Ui) -> R,
        ) -> Option<InnerResponse<Option<R>>> {
            self.show_dyn(ctx, Box::new(add_contents))
        }

        fn show_dyn<'c, R>(
            self,
            ctx: &Context,
            add_contents: Box<dyn FnOnce(&mut Ui) -> R + 'c>,
        ) -> Option<InnerResponse<Option<R>>> {
            let mut ui = Ui::default()
                .with_area(ctx.frame_size())
                .with_ctx(ctx.clone())
                .with_layout(Layout::horizontal([Constraint::Min(1)]).into());

            let inner = add_contents(&mut ui);

            Some(InnerResponse::new(Some(inner), Response::default()))
        }
    }

    #[derive(Clone, Debug)]
    pub struct TableState<R> {
        items: Vec<R>,
        internal: ratatui::widgets::TableState,
    }

    impl<R> TableState<R>
    where
        R: Clone,
    {
        pub fn new(selected: Option<usize>, items: Vec<R>) -> Self {
            let mut internal = ratatui::widgets::TableState::default();
            internal.select(selected);

            Self { items, internal }
        }

        pub fn items(&self) -> &Vec<R> {
            &self.items
        }

        pub fn selected(&self) -> Option<usize> {
            self.internal.selected()
        }
    }

    impl<R> TableState<R>
    where
        R: Clone,
    {
        fn prev(&mut self) -> Option<usize> {
            let selected = self
                .internal
                .selected()
                .map(|current| current.saturating_sub(1));
            self.select(selected);
            selected
        }

        fn next(&mut self, len: usize) -> Option<usize> {
            let selected = self.internal.selected().map(|current| {
                if current < len.saturating_sub(1) {
                    current.saturating_add(1)
                } else {
                    current
                }
            });
            self.select(selected);
            selected
        }

        fn prev_page(&mut self, page_size: usize) -> Option<usize> {
            let selected = self
                .internal
                .selected()
                .map(|current| current.saturating_sub(page_size));
            self.select(selected);
            selected
        }

        fn next_page(&mut self, len: usize, page_size: usize) -> Option<usize> {
            let selected = self.internal.selected().map(|current| {
                if current < len.saturating_sub(1) {
                    cmp::min(current.saturating_add(page_size), len.saturating_sub(1))
                } else {
                    current
                }
            });
            self.select(selected);
            selected
        }

        fn begin(&mut self) {
            self.select(Some(0));
        }

        fn end(&mut self, len: usize) {
            self.select(Some(len.saturating_sub(1)));
        }

        fn select(&mut self, selected: Option<usize>) {
            self.internal.select(selected);
        }
    }

    pub struct Table<'a, R, const W: usize> {
        items: &'a Vec<R>,
        selected: &'a mut Option<usize>,
        columns: Vec<Column<'a>>,
        borders: Option<Borders>,
        show_scrollbar: bool,
        dim: bool,
    }

    impl<'a, R, const W: usize> Table<'a, R, W>
    where
        R: ToRow<W>,
    {
        pub fn new(
            selected: &'a mut Option<usize>,
            items: &'a Vec<R>,
            columns: Vec<Column<'a>>,
            borders: Option<Borders>,
        ) -> Self {
            Self {
                items,
                selected,
                columns,
                borders,
                show_scrollbar: true,
                dim: false,
            }
        }

        pub fn dim(mut self, dim: bool) -> Self {
            self.dim = dim;
            self
        }
    }

    impl<'a, R, const W: usize> Widget for Table<'a, R, W>
    where
        R: ToRow<W> + Clone,
    {
        fn ui(self, ui: &mut Ui, frame: &mut Frame) -> Response {
            let mut response = Response::default();
            let area = ui.next_area().unwrap_or_default();

            let show_scrollbar = self.show_scrollbar && self.items.len() >= area.height.into();
            let has_items = !self.items.is_empty();
            let has_focus = true;

            let mut state = TableState {
                items: self.items.clone(),
                internal: {
                    let mut state = ratatui::widgets::TableState::default();
                    state.select(self.selected.clone());
                    state
                },
            };

            let area = render_block(frame, area, self.borders, ui.theme.border_style);

            if let Some(key) = ui.input_with_key(|_| true) {
                let len = self.items.len();
                let page_size = area.height as usize;

                match key {
                    Key::Up | Key::Char('k') => {
                        state.prev();
                    }
                    Key::Down | Key::Char('j') => {
                        state.next(len);
                    }
                    Key::PageUp => {
                        state.prev_page(page_size);
                    }
                    Key::PageDown => {
                        state.next_page(len, page_size);
                    }
                    Key::Home => {
                        state.begin();
                    }
                    Key::End => {
                        state.end(len);
                    }
                    _ => {}
                }
                response.changed = true;
            }

            let widths: Vec<Constraint> = self
                .columns
                .iter()
                .filter_map(|c| {
                    if !c.skip && c.displayed(area.width as usize) {
                        Some(c.width)
                    } else {
                        None
                    }
                })
                .collect();

            if has_items {
                let [table_area, scroller_area] = Layout::horizontal([
                    Constraint::Min(1),
                    if show_scrollbar {
                        Constraint::Length(1)
                    } else {
                        Constraint::Length(0)
                    },
                ])
                .areas(area);

                let rows = self
                    .items
                    .iter()
                    .map(|item| {
                        let mut cells = vec![];
                        let mut it = self.columns.iter();

                        for cell in item.to_row() {
                            if let Some(col) = it.next() {
                                if !col.skip && col.displayed(area.width as usize) {
                                    cells.push(cell.clone())
                                }
                            } else {
                                continue;
                            }
                        }

                        Row::new(cells)
                    })
                    .collect::<Vec<_>>();

                let table = ratatui::widgets::Table::default()
                    .rows(rows)
                    .widths(widths)
                    .column_spacing(1)
                    .highlight_style(style::highlight(has_focus));

                let table = if !has_focus && self.dim {
                    table.dim()
                } else {
                    table
                };

                frame.render_stateful_widget(table, table_area, &mut state.internal);

                let scroller = Scrollbar::default()
                    .begin_symbol(None)
                    .track_symbol(None)
                    .end_symbol(None)
                    .thumb_symbol("┃")
                    .style(if has_focus {
                        Style::default()
                    } else {
                        Style::default().dim()
                    });

                // In order to make the scrollbar work correctly towards the end of the list,
                // we need to add a few percent of the total length.
                let virtual_length =
                    self.items.len() * ((self.items.len() as f64).log2() as usize) / 100;
                let content_length = area.height as usize + virtual_length;

                let mut scroller_state = ScrollbarState::default()
                    .content_length(self.items.len().saturating_sub(content_length))
                    .viewport_content_length(1)
                    .position(state.internal.offset());

                frame.render_stateful_widget(scroller, scroller_area, &mut scroller_state);
            } else {
                let center = layout::centered_rect(area, 50, 10);
                let hint = Text::from(span::default("Nothing to show"))
                    .centered()
                    .light_magenta()
                    .dim();

                frame.render_widget(hint, center);
            }

            *self.selected = state.selected();

            response
        }
    }

    pub struct Columns<'a> {
        columns: Vec<Column<'a>>,
        borders: Option<Borders>,
    }

    impl<'a> Columns<'a> {
        pub fn new(columns: Vec<Column<'a>>, borders: Option<Borders>) -> Self {
            Self { columns, borders }
        }
    }

    impl<'a> Widget for Columns<'a> {
        fn ui(self, ui: &mut Ui, frame: &mut Frame) -> Response {
            let area = ui.next_area().unwrap_or_default();
            let area = render_block(frame, area, self.borders, ui.theme.border_style);
            let area = Rect {
                width: area.width - 1,
                ..area
            };

            let widths = self
                .columns
                .iter()
                .map(|c| match c.width {
                    Constraint::Min(min) => Constraint::Length(min.saturating_add(3)),
                    _ => c.width,
                })
                .collect::<Vec<_>>();

            let cells = self
                .columns
                .iter()
                .map(|c| c.text.clone())
                .collect::<Vec<_>>();

            let table = ratatui::widgets::Table::default()
                .header(Row::new(cells))
                .widths(widths);
            frame.render_widget(table, area);

            Response::default()
        }
    }

    pub struct TextView {
        text: String,
        borders: Option<Borders>,
    }

    impl TextView {
        pub fn new(text: impl ToString, borders: Option<Borders>) -> Self {
            Self {
                text: text.to_string(),
                borders,
            }
        }
    }

    impl Widget for TextView {
        fn ui(self, ui: &mut Ui, frame: &mut Frame) -> Response {
            let area = ui.next_area().unwrap_or_default();
            let area = render_block(frame, area, self.borders, ui.theme.border_style);

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
        borders: Option<Borders>,
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
        pub fn new(text: &'a mut String, cursor: &'a mut usize, borders: Option<Borders>) -> Self {
            Self {
                text,
                cursor,
                label: None,
                borders,
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
            let area = render_block(frame, area, self.borders, ui.theme.border_style);

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

    fn render_block(frame: &mut Frame, area: Rect, borders: Option<Borders>, style: Style) -> Rect {
        if let Some(border) = borders {
            match border {
                Borders::All => {
                    let block = Block::default()
                        .border_style(style)
                        .border_type(BorderType::Rounded)
                        .borders(ratatui::widgets::Borders::ALL);
                    frame.render_widget(block.clone(), area);

                    block.inner(area)
                }
                Borders::Top => {
                    let block = HeaderBlock::default()
                        .border_style(style)
                        .border_type(BorderType::Rounded)
                        .borders(ratatui::widgets::Borders::ALL);
                    frame.render_widget(block, area);

                    let areas = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints(vec![Constraint::Min(1)])
                        .vertical_margin(1)
                        .horizontal_margin(1)
                        .split(area);

                    areas[0]
                }
                Borders::Sides => {
                    let block = Block::default()
                        .border_style(style)
                        .border_type(BorderType::Rounded)
                        .borders(
                            ratatui::widgets::Borders::LEFT | ratatui::widgets::Borders::RIGHT,
                        );
                    frame.render_widget(block.clone(), area);

                    block.inner(area)
                }
                Borders::Bottom => {
                    let areas = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints(vec![Constraint::Min(1)])
                        .vertical_margin(1)
                        .horizontal_margin(1)
                        .split(area);

                    let footer_block = FooterBlock::default()
                        .border_style(style)
                        .block_type(FooterBlockType::Single { top: true });
                    frame.render_widget(footer_block, area);

                    areas[0]
                }
                Borders::BottomSides => {
                    let areas = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints(vec![Constraint::Min(1)])
                        .horizontal_margin(1)
                        .split(area);

                    let footer_block = FooterBlock::default()
                        .border_style(style)
                        .block_type(FooterBlockType::Single { top: false });
                    frame.render_widget(footer_block, area);

                    Rect {
                        height: areas[0].height.saturating_sub(1),
                        ..areas[0]
                    }
                }
            }
        } else {
            area
        }
    }
}
