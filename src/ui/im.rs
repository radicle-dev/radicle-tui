pub mod widget;

use std::collections::VecDeque;
use std::fmt::Debug;
use std::rc::Rc;
use std::time::Duration;

use anyhow::Result;

use ratatui::text::Text;
use tokio::sync::broadcast;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use termion::event::Key;

use ratatui::layout::{Constraint, Rect};
use ratatui::Frame;

use crate::event::Event;
use crate::store::State;
use crate::task::Interrupted;
use crate::terminal;
use crate::ui::theme::Theme;
use crate::ui::{Column, ToRow};

use crate::ui::im::widget::{HeaderedTable, Widget};

const RENDERING_TICK_RATE: Duration = Duration::from_millis(250);
const INLINE_HEIGHT: usize = 20;

pub trait App {
    type State;
    type Message;

    fn update(
        &self,
        ctx: &Context<Self::Message>,
        frame: &mut Frame,
        state: &Self::State,
    ) -> Result<()>;
}

#[derive(Default)]
pub struct Frontend {}

impl Frontend {
    pub async fn run<S, M, P>(
        self,
        app: impl App<State = S, Message = M>,
        state_tx: UnboundedSender<M>,
        mut state_rx: UnboundedReceiver<S>,
        mut interrupt_rx: broadcast::Receiver<Interrupted<P>>,
    ) -> anyhow::Result<Interrupted<P>>
    where
        S: State<P> + 'static,
        M: Clone + 'static,
        P: Clone + Send + Sync + Debug,
    {
        let mut ticker = tokio::time::interval(RENDERING_TICK_RATE);

        let mut terminal = terminal::setup(INLINE_HEIGHT)?;
        let mut events_rx = terminal::events();

        let mut state = state_rx.recv().await.unwrap();
        let mut ctx = Context::default().with_sender(state_tx);

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

#[derive(Clone, Debug)]
pub struct Context<M> {
    pub(crate) inputs: VecDeque<Key>,
    pub(crate) frame_size: Rect,
    pub(crate) sender: Option<UnboundedSender<M>>,
}

impl<M> Default for Context<M> {
    fn default() -> Self {
        Self {
            inputs: VecDeque::default(),
            frame_size: Rect::default(),
            sender: None,
        }
    }
}

impl<M> Context<M> {
    pub fn new(frame_size: Rect) -> Self {
        Self {
            frame_size,
            ..Default::default()
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

    pub fn with_sender(mut self, sender: UnboundedSender<M>) -> Self {
        self.sender = Some(sender);
        self
    }

    pub fn frame_size(&self) -> Rect {
        self.frame_size
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

    pub fn is_empty(&self) -> bool {
        self.len() == 0
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

#[derive(Clone, Debug)]
pub struct Ui<M> {
    pub theme: Theme,
    pub(crate) area: Rect,
    pub(crate) layout: Layout,
    focus: Option<usize>,
    count: usize,
    ctx: Context<M>,
}

impl<M> Ui<M> {
    pub fn input(&mut self, f: impl Fn(Key) -> bool) -> bool {
        self.has_focus() && self.ctx.inputs.iter().any(|key| f(*key))
    }

    pub fn input_global(&mut self, f: impl Fn(Key) -> bool) -> bool {
        self.ctx.inputs.iter().any(|key| f(*key))
    }

    pub fn input_with_key(&mut self, f: impl Fn(Key) -> bool) -> Option<Key> {
        if self.has_focus() {
            self.ctx.inputs.iter().find(|key| f(**key)).copied()
        } else {
            None
        }
    }
}

impl<M> Default for Ui<M> {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            area: Rect::default(),
            layout: Layout::default(),
            focus: None,
            count: 0,
            ctx: Context::default(),
        }
    }
}

impl<M> Ui<M> {
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

    pub fn with_ctx(mut self, ctx: Context<M>) -> Self {
        self.ctx = ctx;
        self
    }

    // pub fn with_sender(mut self, sender: UnboundedSender<M>) -> Self {
    //     self.sender = Some(sender);
    //     self
    // }

    pub fn area(&self) -> Rect {
        self.area
    }

    pub fn next_area(&mut self) -> Option<(Rect, bool)> {
        let has_focus = self.focus.map(|focus| self.count == focus).unwrap_or(false);
        let rect = self.layout.split(self.area).get(self.count).cloned();

        self.count += 1;

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

    pub fn send_message(&self, message: M) {
        if let Some(sender) = &self.ctx.sender {
            let _ = sender.send(message);
        }
    }
}

impl<M> Ui<M>
where
    M: Clone,
{
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

impl<M> Ui<M>
where
    M: Clone,
{
    pub fn group<R>(
        &mut self,
        layout: impl Into<Layout>,
        focus: &mut Option<usize>,
        add_contents: impl FnOnce(&mut Ui<M>) -> R,
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

    pub fn overline(&mut self, frame: &mut Frame) -> Response {
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

    pub fn columns(
        &mut self,
        frame: &mut Frame,
        columns: Vec<Column<'_>>,
        borders: Option<Borders>,
    ) -> Response {
        widget::Columns::new(columns, borders).ui(self, frame)
    }

    pub fn bar(
        &mut self,
        frame: &mut Frame,
        columns: Vec<Column<'_>>,
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
