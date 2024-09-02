use std::collections::VecDeque;
use std::fmt::Debug;
use std::rc::Rc;
use std::time::Duration;

use anyhow::Result;

use ratatui::text::Text;
use tokio::sync::broadcast;
use tokio::sync::mpsc::UnboundedReceiver;

use termion::event::Key;

use ratatui::layout::{Constraint, Rect};
use ratatui::Frame;

use crate::event::Event;
use crate::store::State;
use crate::task::Interrupted;
use crate::terminal;
use crate::ui::theme::Theme;
use crate::ui::widget::container::Column;
use crate::ui::widget::list::ToRow;

use self::widget::{HeaderedTable, Widget};

const RENDERING_TICK_RATE: Duration = Duration::from_millis(250);
const INLINE_HEIGHT: usize = 20;

pub trait App {
    type State;
    type Message;

    fn update(&self, ui: &Context, frame: &mut Frame, state: &Self::State) -> Result<()>;
}

#[derive(Default)]
pub struct Frontend {}

impl Frontend {
    pub async fn main_loop<S, M, P>(
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
    None,
    Spacer { top: usize, left: usize },
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
    pub fn len(&self) -> usize {
        match self {
            Layout::None => 0,
            Layout::Wrapped { internal } => internal.split(Rect::default()).len(),
            Layout::Expandable3 { left_only } => {
                if *left_only {
                    1
                } else {
                    3
                }
            }
        }
    }

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
    pub theme: Theme,
    pub(crate) area: Rect,
    pub(crate) layout: Layout,
    focus: Option<usize>,
    count: usize,
    ctx: Context,
}

impl Ui {
    pub fn input(&mut self, f: impl Fn(Key) -> bool) -> bool {
        self.has_focus() && self.ctx.inputs.iter().find(|key| f(**key)).is_some()
    }

    pub fn input_global(&mut self, f: impl Fn(Key) -> bool) -> bool {
        self.ctx.inputs.iter().find(|key| f(**key)).is_some()
    }

    pub fn input_with_key(&mut self, f: impl Fn(Key) -> bool) -> Option<Key> {
        if self.has_focus() {
            self.ctx.inputs.iter().find(|key| f(**key)).copied()
        } else {
            None
        }
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

    pub fn next_area(&mut self) -> Option<(Rect, bool)> {
        let has_focus = self.focus.map(|focus| self.count == focus).unwrap_or(false);
        let rect = self.layout.split(self.area).get(self.count).cloned();

        self.count = self.count + 1;

        rect.map(|rect| (rect, has_focus))
    }

    pub fn current_area(&mut self) -> Option<(Rect, bool)> {
        let count = self.count.saturating_sub(1);

        let has_focus = self.focus.map(|focus| count == focus).unwrap_or(false);
        let rect = self.layout.split(self.area).get(self.count).cloned();

        rect.map(|rect| (rect, has_focus))
    }

    pub fn has_focus(&self) -> bool {
        let count = self.count.saturating_sub(1);
        self.focus.map(|focus| count == focus).unwrap_or(false)
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn set_focus(&mut self, focus: Option<usize>) {
        self.focus = focus;
    }

    pub fn focus_next(&mut self) {
        if self.focus.is_none() {
            self.focus = Some(0);
        } else {
            self.focus = Some(self.focus.unwrap().saturating_add(1));
        }
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
        let (area, _) = self.next_area().unwrap_or_default();
        let mut child_ui = self.child_ui(area, layout);
        let inner = add_contents(&mut child_ui);

        InnerResponse::new(inner, Response::default())
    }
}

impl Ui {
    pub fn group<R>(
        &mut self,
        layout: impl Into<Layout>,
        focus: &mut Option<usize>,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) -> InnerResponse<R> {
        let (area, _) = self.next_area().unwrap_or_default();

        let layout: Layout = layout.into();
        let len = layout.len();

        let mut child_ui = self.child_ui(area, layout);
        child_ui.set_focus(Some(0));

        widget::Group::new(len, focus).show(&mut child_ui, add_contents)
    }

    pub fn label<'a>(&mut self, frame: &mut Frame, content: impl Into<Text<'a>>) -> Response {
        widget::Label::new(content).ui(self, frame)
    }

    pub fn overline<'a>(&mut self, frame: &mut Frame) -> Response {
        let overline = String::from("━").repeat(256);
        self.label(frame, overline)
    }

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

    pub fn headered_table<'a, R, const W: usize>(
        &mut self,
        frame: &mut Frame,
        selected: &'a mut Option<usize>,
        items: &'a Vec<R>,
        header: impl IntoIterator<Item = Column<'a>>,
    ) -> Response
    where
        R: ToRow<W> + Clone,
    {
        HeaderedTable::<R, W>::new(selected, items, header).ui(self, frame)
    }

    pub fn shortcuts(
        &mut self,
        frame: &mut Frame,
        shortcuts: &[(&str, &str)],
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

    pub fn bar<'a>(
        &mut self,
        frame: &mut Frame,
        columns: Vec<Column<'a>>,
        borders: Option<Borders>,
    ) -> Response {
        widget::Bar::new(columns, borders).ui(self, frame)
    }

    pub fn text_view(
        &mut self,
        frame: &mut Frame,
        text: String,
        scroll: &mut (usize, usize),
        borders: Option<Borders>,
    ) -> Response {
        widget::TextView::new(text, scroll, borders).ui(self, frame)
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
    pub struct GroupState {
        len: usize,
        focus: Option<usize>,
    }

    impl GroupState {
        pub fn new(len: usize, focus: Option<usize>) -> Self {
            Self { len, focus }
        }

        pub fn focus(&self) -> Option<usize> {
            self.focus
        }

        pub fn len(&self) -> usize {
            self.len
        }

        pub fn focus_next(&mut self) {
            self.focus = self
                .focus
                .map(|focus| cmp::min(focus.saturating_add(1), self.len.saturating_sub(1)))
        }

        pub fn focus_prev(&mut self) {
            self.focus = self.focus.map(|focus| focus.saturating_sub(1))
        }
    }

    pub struct Group<'a> {
        focus: &'a mut Option<usize>,
        len: usize,
    }

    impl<'a> Group<'a> {
        pub fn new(len: usize, focus: &'a mut Option<usize>) -> Self {
            Self { len, focus }
        }

        pub fn show<R>(
            self,
            ui: &mut Ui,
            add_contents: impl FnOnce(&mut Ui) -> R,
        ) -> InnerResponse<R> {
            self.show_dyn(ui, Box::new(add_contents))
        }

        pub fn show_dyn<'c, R>(
            self,
            ui: &mut Ui,
            add_contents: Box<dyn FnOnce(&mut Ui) -> R + 'c>,
        ) -> InnerResponse<R> {
            let mut response = Response::default();

            let mut state = GroupState {
                focus: *self.focus,
                len: self.len,
            };

            if let Some(key) = ui.input_with_key(|_| true) {
                match key {
                    Key::Char('\t') => {
                        state.focus_next();
                        response.changed = true;
                    }
                    Key::BackTab => {
                        state.focus_prev();
                        response.changed = true;
                    }
                    _ => {}
                }
            }
            *self.focus = state.focus;

            let mut ui = Ui {
                focus: state.focus,
                ..ui.clone()
            };

            let inner = add_contents(&mut ui);

            InnerResponse::new(inner, response)
        }
    }

    pub struct Label<'a> {
        content: Text<'a>,
    }

    impl<'a> Label<'a> {
        pub fn new(content: impl Into<Text<'a>>) -> Self {
            Self {
                content: content.into(),
            }
        }
    }

    impl<'a> Widget for Label<'a> {
        fn ui(self, ui: &mut Ui, frame: &mut Frame) -> Response {
            let (area, _) = ui.next_area().unwrap_or_default();
            frame.render_widget(self.content, area);

            Response::default()
        }
    }

    #[derive(Clone, Debug)]
    pub struct TableState {
        internal: ratatui::widgets::TableState,
    }

    impl TableState {
        pub fn new(selected: Option<usize>) -> Self {
            let mut internal = ratatui::widgets::TableState::default();
            internal.select(selected);

            Self { internal }
        }

        pub fn selected(&self) -> Option<usize> {
            self.internal.selected()
        }

        pub fn select_first(&mut self) {
            self.internal.select(Some(0));
        }
    }

    impl TableState {
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

            let (area, has_focus) = ui.next_area().unwrap_or_default();

            let show_scrollbar = self.show_scrollbar && self.items.len() >= area.height.into();
            let has_items = !self.items.is_empty();

            let mut state = TableState {
                internal: {
                    let mut state = ratatui::widgets::TableState::default();
                    state.select(self.selected.clone());
                    state
                },
            };

            let border_style = if has_focus {
                ui.theme.focus_border_style
            } else {
                ui.theme.border_style
            };

            let area = render_block(frame, area, self.borders, border_style);

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

                if show_scrollbar {
                    let content_length = self.items.len();
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

                    let mut state = ScrollbarState::default()
                        .content_length(content_length)
                        .viewport_content_length(1)
                        .position(state.internal.offset());

                    frame.render_stateful_widget(scroller, scroller_area, &mut state);
                }
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

    pub struct HeaderedTable<'a, R, const W: usize> {
        items: &'a Vec<R>,
        selected: &'a mut Option<usize>,
        header: Vec<Column<'a>>,
    }

    impl<'a, R, const W: usize> HeaderedTable<'a, R, W> {
        pub fn new(
            selected: &'a mut Option<usize>,
            items: &'a Vec<R>,
            header: impl IntoIterator<Item = Column<'a>>,
        ) -> Self {
            Self {
                items,
                selected,
                header: header.into_iter().collect(),
            }
        }

        pub fn items(&self) -> &Vec<R> {
            &self.items
        }
    }

    /// TODO(erikli): Implement `show` that returns an `InnerResponse` such that it can
    /// used like a group.
    impl<'a, R, const W: usize> Widget for HeaderedTable<'a, R, W>
    where
        R: ToRow<W> + Clone,
    {
        fn ui(self, ui: &mut Ui, frame: &mut Frame) -> Response {
            let mut response = Response::default();

            let (_, has_focus) = ui.current_area().unwrap_or_default();

            ui.layout(
                Layout::vertical([Constraint::Length(3), Constraint::Min(1)]),
                |ui| {
                    // TODO(erikli): Find better solution for border focus workaround or improve
                    // interface for manually advancing / setting the focus index.
                    if has_focus {
                        ui.set_focus(Some(0));
                    }
                    ui.columns(frame, self.header.clone().to_vec(), Some(Borders::Top));

                    if has_focus {
                        ui.set_focus(Some(1));
                    }
                    let table = ui.table(
                        frame,
                        self.selected,
                        &self.items,
                        self.header.to_vec(),
                        Some(Borders::BottomSides),
                    );
                    response.changed = table.changed | response.changed;
                },
            );

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
            let (area, has_focus) = ui.next_area().unwrap_or_default();

            let border_style = if has_focus {
                ui.theme.focus_border_style
            } else {
                ui.theme.border_style
            };

            let area = render_block(frame, area, self.borders, border_style);
            let area = Rect {
                width: area.width.saturating_sub(1),
                ..area
            };

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

    pub struct Bar<'a> {
        columns: Vec<Column<'a>>,
        borders: Option<Borders>,
    }

    impl<'a> Bar<'a> {
        pub fn new(columns: Vec<Column<'a>>, borders: Option<Borders>) -> Self {
            Self { columns, borders }
        }
    }

    impl<'a> Widget for Bar<'a> {
        fn ui(self, ui: &mut Ui, frame: &mut Frame) -> Response {
            let (area, has_focus) = ui.next_area().unwrap_or_default();

            let border_style = if has_focus {
                ui.theme.focus_border_style
            } else {
                ui.theme.border_style
            };

            let widths = self.columns.iter().map(|c| c.width).collect::<Vec<_>>();
            let cells = self
                .columns
                .iter()
                .map(|c| c.text.clone())
                .collect::<Vec<_>>();

            let area = render_block(frame, area, self.borders, border_style);
            let table = ratatui::widgets::Table::default()
                .header(Row::new(cells))
                .widths(widths)
                .column_spacing(0);
            frame.render_widget(table, area);

            Response::default()
        }
    }

    #[derive(Clone, Debug)]
    pub struct TextViewState {
        text: String,
        cursor: (usize, usize),
    }

    impl TextViewState {
        pub fn new(text: impl Into<String>, cursor: (usize, usize)) -> Self {
            Self {
                text: text.into(),
                cursor,
            }
        }

        pub fn text(&self) -> &String {
            &self.text
        }

        pub fn cursor(&self) -> (usize, usize) {
            self.cursor
        }
    }

    impl TextViewState {
        fn scroll_up(&mut self) {
            self.cursor.0 = self.cursor.0.saturating_sub(1);
        }

        fn scroll_down(&mut self, len: usize, page_size: usize) {
            let end = len.saturating_sub(page_size);
            self.cursor.0 = std::cmp::min(self.cursor.0.saturating_add(1), end);
        }

        fn scroll_left(&mut self) {
            self.cursor.1 = self.cursor.1.saturating_sub(3);
        }

        fn scroll_right(&mut self, max_line_length: usize) {
            self.cursor.1 = std::cmp::min(
                self.cursor.1.saturating_add(3),
                max_line_length.saturating_add(3),
            );
        }

        fn prev_page(&mut self, page_size: usize) {
            self.cursor.0 = self.cursor.0.saturating_sub(page_size);
        }

        fn next_page(&mut self, len: usize, page_size: usize) {
            let end = len.saturating_sub(page_size);

            self.cursor.0 = std::cmp::min(self.cursor.0.saturating_add(page_size), end);
        }

        fn begin(&mut self) {
            self.cursor.0 = 0;
        }

        fn end(&mut self, len: usize, page_size: usize) {
            self.cursor.0 = len.saturating_sub(page_size);
        }
    }

    pub struct TextView<'a> {
        text: String,
        borders: Option<Borders>,
        cursor: &'a mut (usize, usize),
    }

    impl<'a> TextView<'a> {
        pub fn new(
            text: impl ToString,
            cursor: &'a mut (usize, usize),
            borders: Option<Borders>,
        ) -> Self {
            Self {
                text: text.to_string(),
                borders,
                cursor,
            }
        }
    }

    impl<'a> Widget for TextView<'a> {
        fn ui(self, ui: &mut Ui, frame: &mut Frame) -> Response {
            let mut response = Response::default();

            let (area, has_focus) = ui.next_area().unwrap_or_default();

            let show_scrollbar = true;
            let border_style = if has_focus {
                ui.theme.focus_border_style
            } else {
                ui.theme.border_style
            };
            let length = self.text.lines().count();
            // let virtual_length = length * ((length as f64).log2() as usize) / 100;
            // let content_length = area.height as usize + virtual_length;
            // let content_length = length;
            let content_length = area.height as usize;

            let area = render_block(frame, area, self.borders, border_style);
            let area = Rect {
                x: area.x.saturating_add(1),
                width: area.width.saturating_sub(1),
                ..area
            };
            let [text_area, scroller_area] = Layout::horizontal([
                Constraint::Min(1),
                if show_scrollbar {
                    Constraint::Length(1)
                } else {
                    Constraint::Length(0)
                },
            ])
            .areas(area);

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

            let mut scroller_state = ScrollbarState::default()
                .content_length(length.saturating_sub(content_length))
                .viewport_content_length(1)
                .position(self.cursor.0);

            frame.render_stateful_widget(scroller, scroller_area, &mut scroller_state);
            frame.render_widget(
                Paragraph::new(self.text.clone())
                    .scroll((self.cursor.0 as u16, self.cursor.1 as u16)),
                text_area,
            );

            let mut state = TextViewState::new(self.text.clone(), *self.cursor);

            if let Some(key) = ui.input_with_key(|_| true) {
                let lines = self.text.lines().clone();
                let len = lines.clone().count();
                let max_line_len = lines.map(|l| l.chars().count()).max().unwrap_or_default();
                let page_size = area.height as usize;

                match key {
                    Key::Up | Key::Char('k') => {
                        state.scroll_up();
                    }
                    Key::Down | Key::Char('j') => {
                        state.scroll_down(len, page_size);
                    }
                    Key::Left | Key::Char('h') => {
                        state.scroll_left();
                    }
                    Key::Right | Key::Char('l') => {
                        state.scroll_right(max_line_len.saturating_sub(area.height.into()));
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
                        state.end(len, page_size);
                    }
                    _ => {}
                }
                *self.cursor = state.cursor;
                response.changed = true;
            }

            response
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
            new_cursor_pos.clamp(0, self.text.len())
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

            let (area, has_focus) = ui.next_area().unwrap_or_default();

            let border_style = if has_focus {
                ui.theme.focus_border_style
            } else {
                ui.theme.border_style
            };

            let area = render_block(frame, area, self.borders, border_style);

            let layout = Layout::vertical(Constraint::from_lengths([1, 1])).split(area);

            let mut state = TextEditState {
                text: self.text.to_string(),
                cursor: *self.cursor,
            };

            let label_content = format!(" {} ", self.label.unwrap_or_default());
            let overline = String::from("▔").repeat(area.width as usize);
            let cursor_pos = *self.cursor as u16;

            let (label, input, overline) = if !has_focus && self.dim {
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
        pub fn new(shortcuts: &[(&str, &str)], divider: char) -> Self {
            Self {
                shortcuts: shortcuts
                    .iter()
                    .map(|(s, a)| (s.to_string(), a.to_string()))
                    .collect(),
                divider,
            }
        }
    }

    impl Widget for Shortcuts {
        fn ui(self, ui: &mut Ui, frame: &mut Frame) -> Response {
            use ratatui::widgets::Table;

            let (area, _) = ui.next_area().unwrap_or_default();

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

            frame.render_widget(table, area);

            Response::default()
        }
    }

    fn render_block(frame: &mut Frame, area: Rect, borders: Option<Borders>, style: Style) -> Rect {
        if let Some(border) = borders {
            match border {
                Borders::None => area,
                Borders::Spacer { top, left } => {
                    let areas = Layout::horizontal([Constraint::Fill(1)])
                        .vertical_margin(top as u16)
                        .horizontal_margin(left as u16)
                        .split(area);

                    areas[0]
                }
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

/// A `BufferedValue` that writes updates to an internal
/// buffer. This buffer can be applied or reset.
///
/// Reading from a `BufferedValue` will return the buffer if it's
/// not empty. It will return the actual value otherwise.
#[derive(Clone, Debug)]
pub struct BufferedValue<T>
where
    T: Clone,
{
    value: T,
    buffer: Option<T>,
}

impl<T> BufferedValue<T>
where
    T: Clone,
{
    pub fn new(value: T) -> Self {
        Self {
            value,
            buffer: None,
        }
    }

    pub fn apply(&mut self) {
        if let Some(buffer) = self.buffer.clone() {
            self.value = buffer;
        }
        self.buffer = None;
    }

    pub fn reset(&mut self) {
        self.buffer = None;
    }

    pub fn write(&mut self, value: T) {
        self.buffer = Some(value);
    }

    pub fn read(&self) -> T {
        if let Some(buffer) = self.buffer.clone() {
            buffer
        } else {
            self.value.clone()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn state_value_read_should_succeed() {
        let value = BufferedValue::new(0);
        assert_eq!(value.read(), 0);
    }

    #[test]
    fn state_value_read_buffer_should_succeed() {
        let mut value = BufferedValue::new(0);
        value.write(1);

        assert_eq!(value.read(), 1);
    }

    #[test]
    fn state_value_apply_should_succeed() {
        let mut value = BufferedValue::new(0);

        value.write(1);
        assert_eq!(value.read(), 1);

        value.apply();
        assert_eq!(value.read(), 1);
    }

    #[test]
    fn state_value_reset_should_succeed() {
        let mut value = BufferedValue::new(0);

        value.write(1);
        assert_eq!(value.read(), 1);

        value.reset();
        assert_eq!(value.read(), 0);
    }

    #[test]
    fn state_value_reset_after_apply_should_succeed() {
        let mut value = BufferedValue::new(0);

        value.write(1);
        assert_eq!(value.read(), 1);

        value.apply();
        value.reset();
        assert_eq!(value.read(), 1);
    }
}
