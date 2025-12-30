pub mod widget;

use std::collections::{HashSet, VecDeque};
use std::fmt::Debug;
use std::hash::Hash;
use std::rc::Rc;
use std::time::Duration;

use anyhow::Result;

use ratatui::style::Stylize;
use ratatui::text::{Span, Text};
use tokio::sync::broadcast;
use tokio::sync::mpsc::UnboundedReceiver;

use ratatui::layout::{Alignment, Constraint, Flex, Position, Rect};
use ratatui::{Frame, Viewport};

use crate::event::{Event, Key};
use crate::store::Update;
use crate::terminal::Terminal;
use crate::ui::theme::Theme;
use crate::ui::{Column, Spacing, ToRow, ToTree};
use crate::{Interrupted, Share};

use crate::ui::im::widget::Widget;

use self::widget::AddContentFn;

use super::layout;

const RENDERING_TICK_RATE: Duration = Duration::from_millis(250);

/// The main UI trait for the ability to render an application.
pub trait Show<M> {
    fn show(&self, ctx: &Context<M>, frame: &mut Frame) -> Result<()>;
}

#[derive(Default)]
pub struct Frontend {}

impl Frontend {
    pub async fn run<S, M, R>(
        self,
        message_tx: broadcast::Sender<M>,
        mut state_rx: UnboundedReceiver<S>,
        mut event_rx: UnboundedReceiver<Event>,
        mut interrupt_rx: broadcast::Receiver<Interrupted<R>>,
        viewport: Viewport,
    ) -> anyhow::Result<Interrupted<R>>
    where
        S: Update<M, Return = R> + Show<M>,
        M: Share,
        R: Share,
    {
        let mut ticker = tokio::time::interval(RENDERING_TICK_RATE);
        let mut terminal = Terminal::try_from(viewport)?;

        let mut state = state_rx.recv().await.unwrap();
        let mut ctx = Context::default().with_sender(message_tx);

        let result: anyhow::Result<Interrupted<R>> = loop {
            tokio::select! {
                // Tick to terminate the select every N milliseconds
                _ = ticker.tick() => (),
                // Handle input events
                Some(event) = event_rx.recv() => {
                    match event {
                        Event::Key(key) => {
                            log::debug!(target: "frontend", "Received key event: {key:?}");
                            ctx.store_input(event)
                        }
                        Event::Resize(x, y) => {
                            log::debug!(target: "frontend", "Received resize event: {x},{y}");
                            terminal.clear()?;
                        },
                        Event::Unknown => {
                            log::debug!(target: "frontend", "Received unknown event")
                        }
                    }
                },
                // Handle state updates
                Some(s) = state_rx.recv() => {
                    state = s;
                },
                // Catch and handle interrupt signal to gracefully shutdown
                Ok(interrupted) = interrupt_rx.recv() => {
                    break Ok(interrupted);
                }
            }
            terminal.draw(|frame| {
                let ctx = ctx.clone().with_frame_size(frame.area());

                if let Err(err) = state.show(&ctx, frame) {
                    log::warn!("Drawing failed: {err}");
                }
            })?;

            ctx.clear_inputs();
        };
        terminal.restore()?;

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

/// A `Context` is held by the `Ui` and reflects the environment a `Ui` runs in.
#[derive(Clone, Debug)]
pub struct Context<M> {
    /// Currently captured user inputs. Inputs that where stored via `store_input`
    /// need to be cleared manually via `clear_inputs` (usually for each frame drawn).
    inputs: VecDeque<Event>,
    /// Current frame of the application.
    pub(crate) frame_size: Rect,
    /// The message sender used by the `Ui` to send application messages.
    pub(crate) sender: Option<broadcast::Sender<M>>,
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

    pub fn with_inputs(mut self, inputs: VecDeque<Event>) -> Self {
        self.inputs = inputs;
        self
    }

    pub fn with_frame_size(mut self, frame_size: Rect) -> Self {
        self.frame_size = frame_size;
        self
    }

    pub fn with_sender(mut self, sender: broadcast::Sender<M>) -> Self {
        self.sender = Some(sender);
        self
    }

    pub fn frame_size(&self) -> Rect {
        self.frame_size
    }

    pub fn store_input(&mut self, event: Event) {
        self.inputs.push_back(event);
    }

    pub fn clear_inputs(&mut self) {
        self.inputs.clear();
    }
}

/// `Borders` defines which borders should be drawn around a widget.
pub enum Borders {
    None,
    Spacer { top: usize, left: usize },
    All,
    Top,
    Sides,
    Bottom,
    BottomSides,
}

/// A `Layout` is used to support pre-defined layouts. It either represents
/// such a predefined layout or a wrapped `ratatui` layout. It's used internally
/// but can be build from a `ratatui` layout.
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
    Popup {
        percent_x: u16,
        percent_y: u16,
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
            Layout::Popup {
                percent_x: _,
                percent_y: _,
            } => 1,
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
            Layout::Popup {
                percent_x,
                percent_y,
            } => {
                use ratatui::layout::Layout;

                let vertical =
                    Layout::vertical([Constraint::Percentage(*percent_y)]).flex(Flex::Center);
                let horizontal =
                    Layout::horizontal([Constraint::Percentage(*percent_x)]).flex(Flex::Center);
                let [area] = vertical.areas(area);
                let [area] = horizontal.areas(area);

                [area].into()
            }
        }
    }
}

/// The `Ui` is the main frontend component that provides render and user-input capture
/// capabilities. An application consists of at least 1 root `Ui`. An `Ui` can build child
/// `Ui`s that partially inherit attributes.
#[derive(Clone, Debug)]
pub struct Ui<M> {
    /// The context this runs in: frame sizes, captured user-inputs etc.
    ctx: Context<M>,
    /// The UI theme.
    theme: Theme,
    /// The area this can render in.
    area: Rect,
    /// The layout used to calculate the next area to draw.
    layout: Layout,
    /// Currently focused area.
    focus_area: Option<usize>,
    /// If this has focus.
    has_focus: bool,
    /// Current rendering counter that is increased whenever the next area to draw
    /// on is requested.
    count: usize,
}

impl<M> Ui<M> {
    pub fn has_input(&mut self, f: impl Fn(Key) -> bool) -> bool {
        self.has_focus
            && self.is_area_focused()
            && self.ctx.inputs.iter().any(|event| {
                if let Event::Key(key) = event {
                    return f(*key);
                }
                false
            })
    }

    pub fn get_input(&mut self, f: impl Fn(Key) -> bool) -> Option<Key> {
        if self.has_focus && self.is_area_focused() {
            let matches = |&event| {
                if let Event::Key(key) = event {
                    return f(key);
                }
                false
            };

            if let Some(Event::Key(key)) =
                self.ctx.inputs.iter().find(|event| matches(event)).copied()
            {
                return Some(key);
            }
            None
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
            focus_area: None,
            has_focus: true,
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

    pub fn with_area_focus(mut self, focus: Option<usize>) -> Self {
        self.focus_area = focus;
        self
    }

    pub fn with_ctx(mut self, ctx: Context<M>) -> Self {
        self.ctx = ctx;
        self
    }

    pub fn with_focus(mut self) -> Self {
        self.has_focus = true;
        self
    }

    pub fn without_focus(mut self) -> Self {
        self.has_focus = false;
        self
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn area(&self) -> Rect {
        self.area
    }

    pub fn next_area(&mut self) -> Option<(Rect, bool)> {
        let area_focus = self
            .focus_area
            .map(|focus| self.count == focus)
            .unwrap_or(false);
        let rect = self.layout.split(self.area).get(self.count).cloned();

        self.count += 1;

        rect.map(|rect| (rect, area_focus))
    }

    pub fn current_area(&mut self) -> Option<(Rect, bool)> {
        let count = self.count.saturating_sub(1);

        let area_focus = self.focus_area.map(|focus| count == focus).unwrap_or(false);
        let rect = self.layout.split(self.area).get(self.count).cloned();

        rect.map(|rect| (rect, area_focus))
    }

    pub fn is_area_focused(&self) -> bool {
        let count = self.count.saturating_sub(1);
        self.focus_area.map(|focus| count == focus).unwrap_or(false)
    }

    pub fn has_focus(&self) -> bool {
        self.has_focus
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn focus_next(&mut self) {
        if self.focus_area.is_none() {
            self.focus_area = Some(0);
        } else {
            self.focus_area = Some(self.focus_area.unwrap().saturating_add(1));
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
        focus: Option<usize>,
        add_contents: impl FnOnce(&mut Self) -> R,
    ) -> InnerResponse<R> {
        self.layout_dyn(layout, focus, Box::new(add_contents))
    }

    pub fn layout_dyn<R>(
        &mut self,
        layout: impl Into<Layout>,
        focus: Option<usize>,
        add_contents: Box<AddContentFn<M, R>>,
    ) -> InnerResponse<R> {
        let (area, area_focus) = self.next_area().unwrap_or_default();

        let mut child_ui = Ui {
            has_focus: area_focus,
            focus_area: focus,
            ..self.child_ui(area, layout)
        };

        InnerResponse::new(add_contents(&mut child_ui), Response::default())
    }
}

impl<M> Ui<M>
where
    M: Clone,
{
    pub fn container<R>(
        &mut self,
        layout: impl Into<Layout>,
        focus: &mut Option<usize>,
        add_contents: impl FnOnce(&mut Ui<M>) -> R,
    ) -> InnerResponse<R> {
        let (area, area_focus) = self.next_area().unwrap_or_default();

        let layout: Layout = layout.into();
        let len = layout.len();

        // TODO(erikli): Check if setting the focus area is needed at all.
        let mut child_ui = Ui {
            has_focus: area_focus,
            focus_area: *focus,
            ..self.child_ui(area, layout)
        };

        widget::Container::new(len, focus).show(&mut child_ui, add_contents)
    }

    pub fn popup<R>(
        &mut self,
        layout: impl Into<Layout>,
        add_contents: impl FnOnce(&mut Ui<M>) -> R,
    ) -> InnerResponse<R> {
        let layout: Layout = layout.into();
        let areas = layout.split(self.area());
        let area = areas.first().cloned().unwrap_or(self.area());

        let mut child_ui = self.child_ui(area, layout::fill());
        child_ui.has_focus = true;

        widget::Popup::default().show(&mut child_ui, add_contents)
    }

    pub fn label<'a>(&mut self, frame: &mut Frame, content: impl Into<Text<'a>>) -> Response {
        widget::Label::new(content).ui(self, frame)
    }

    pub fn overline(&mut self, frame: &mut Frame) -> Response {
        let overline = String::from("▔").repeat(256);
        self.label(frame, Span::raw(overline).cyan())
    }

    pub fn separator(&mut self, frame: &mut Frame) -> Response {
        let overline = String::from("─").repeat(256);
        self.label(
            frame,
            Span::raw(overline).fg(self.theme.border_style.fg.unwrap_or_default()),
        )
    }

    pub fn table<'a, R, const W: usize>(
        &mut self,
        frame: &mut Frame,
        selected: &mut Option<usize>,
        items: &'a Vec<R>,
        columns: Vec<Column<'a>>,
        empty_message: Option<String>,
        spacing: Spacing,
        borders: Option<Borders>,
    ) -> Response
    where
        R: ToRow<W> + Clone,
    {
        widget::Table::new(selected, items, columns, empty_message, borders)
            .spacing(spacing)
            .ui(self, frame)
    }

    pub fn tree<'a, R, Id>(
        &mut self,
        frame: &mut Frame,
        items: &'a Vec<R>,
        opened: &mut Option<HashSet<Vec<Id>>>,
        selected: &mut Option<Vec<Id>>,
        borders: Option<Borders>,
    ) -> Response
    where
        R: ToTree<Id> + Clone,
        Id: ToString + Clone + Eq + Hash,
    {
        widget::Tree::new(items, opened, selected, borders, false).ui(self, frame)
    }

    pub fn shortcuts(
        &mut self,
        frame: &mut Frame,
        shortcuts: &[(&str, &str)],
        divider: char,
        alignment: Alignment,
    ) -> Response {
        widget::Shortcuts::new(shortcuts, divider, alignment).ui(self, frame)
    }

    pub fn column_bar(
        &mut self,
        frame: &mut Frame,
        columns: Vec<Column<'_>>,
        spacing: Spacing,
        borders: Option<Borders>,
    ) -> Response {
        widget::ColumnBar::new(columns, spacing, borders).ui(self, frame)
    }

    pub fn text_view<'a>(
        &mut self,
        frame: &mut Frame,
        text: impl Into<Text<'a>>,
        scroll: &'a mut Position,
        borders: Option<Borders>,
    ) -> Response {
        widget::TextView::new(text, None::<String>, scroll, borders).ui(self, frame)
    }

    pub fn text_view_with_footer<'a>(
        &mut self,
        frame: &mut Frame,
        text: impl Into<Text<'a>>,
        footer: impl Into<Text<'a>>,
        scroll: &'a mut Position,
        borders: Option<Borders>,
    ) -> Response {
        widget::TextView::new(text, Some(footer), scroll, borders).ui(self, frame)
    }

    pub fn centered_text_view<'a>(
        &mut self,
        frame: &mut Frame,
        text: impl Into<Text<'a>>,
        borders: Option<Borders>,
    ) -> Response {
        widget::CenteredTextView::new(text, borders).ui(self, frame)
    }

    pub fn text_edit_singleline(
        &mut self,
        frame: &mut Frame,
        text: &mut String,
        cursor: &mut usize,
        label: Option<impl ToString>,
        borders: Option<Borders>,
    ) -> Response {
        match label {
            Some(label) => widget::TextEdit::new(text, cursor, borders)
                .with_label(label)
                .ui(self, frame),
            _ => widget::TextEdit::new(text, cursor, borders).ui(self, frame),
        }
    }
}
