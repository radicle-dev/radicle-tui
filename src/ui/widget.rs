pub mod container;
pub mod input;
pub mod text;

use std::any::Any;
use std::cmp;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::prelude::*;
use ratatui::widgets::{Cell, Row, TableState};

use super::theme::style;
use super::{layout, span};

pub type BoxedWidget<S, A> = Box<dyn Widget<State = S, Action = A>>;

pub type UpdateCallback<S> = fn(&S) -> Box<dyn Any>;
pub type EventCallback<A> = fn(Box<dyn Any>, UnboundedSender<A>);

/// A `View`s common fields.
pub struct BaseView<S, A> {
    /// Message sender
    pub action_tx: UnboundedSender<A>,
    /// Custom update handler
    pub on_update: Option<UpdateCallback<S>>,
    /// Additional custom event handler
    pub on_event: Option<EventCallback<A>>,
}

/// Main trait defining a `Widget` behaviour.
///
/// This is the trait that you should implement to define a custom `Widget`.
pub trait Widget {
    type State;
    type Action;

    /// Should return a new view with props build from state (if type is known) and a
    /// message sender set.
    fn new(state: &Self::State, action_tx: UnboundedSender<Self::Action>) -> Self
    where
        Self: Sized;

    /// Should handle key events and call `handle_event` on all children.
    ///
    /// After key events have been handled, the custom event handler `on_event` should
    /// be called
    fn handle_event(&mut self, key: Key);

    /// Should update the internal props of this and all children.
    ///
    /// Applications are usually defined by app-specific widgets that do know
    /// the type of `state`. These can use widgets from the library that do not know the
    /// type of `state`.
    ///
    /// If `on_update` is set, implementations of this function should call it to
    /// construct and update the internal props. If it is not set, app widgets can construct
    /// props directly via their state converters, whereas library widgets can just fallback
    /// to their current props.
    fn update(&mut self, state: &Self::State);

    /// Renders a widget to the given frame in the given area.
    ///
    /// Optional props take precedence over the internal ones.
    fn render(&self, frame: &mut Frame, area: Rect, props: Option<&dyn Any>);

    /// Return a mutable reference to this widgets' base view.
    fn base_mut(&mut self) -> &mut BaseView<Self::State, Self::Action>;

    /// Should set the optional custom event handler.
    fn on_event(mut self, callback: EventCallback<Self::Action>) -> Self
    where
        Self: Sized,
    {
        self.base_mut().on_event = Some(callback);
        self
    }

    /// Should set the optional update handler.
    fn on_update(mut self, callback: UpdateCallback<Self::State>) -> Self
    where
        Self: Sized,
    {
        self.base_mut().on_update = Some(callback);
        self
    }

    /// Returns a boxed `Widget`
    fn to_boxed(self) -> Box<Self>
    where
        Self: Sized,
    {
        Box::new(self)
    }
}

/// Needs to be implemented for items that are supposed to be rendered in tables.
pub trait ToRow<const W: usize> {
    fn to_row(&self) -> [Cell; W];
}

/// Common trait for view properties.
pub trait Properties {
    fn to_boxed(self) -> Box<Self>
    where
        Self: Sized,
    {
        Box::new(self)
    }

    fn from_boxed_any(any: Box<dyn Any>) -> Option<Self>
    where
        Self: Sized + Clone + 'static,
    {
        any.downcast_ref::<Self>().cloned()
    }

    fn from_callback<S>(callback: Option<UpdateCallback<S>>, state: &S) -> Option<Self>
    where
        Self: Sized + Clone + 'static,
    {
        callback
            .map(|callback| (callback)(state))
            .and_then(|props| Self::from_boxed_any(props))
    }
}

pub trait WidgetState {
    fn to_boxed_any(self) -> Box<dyn Any>
    where
        Self: Sized + Clone + 'static,
    {
        Box::new(self)
    }

    fn from_boxed_any(any: Box<dyn Any>) -> Option<Self>
    where
        Self: Sized + Clone + 'static,
    {
        any.downcast_ref::<Self>().cloned()
    }
}

#[derive(Clone)]
pub struct WindowProps<Id> {
    current_page: Option<Id>,
}

impl<Id> WindowProps<Id> {
    pub fn current_page(mut self, page: Id) -> Self {
        self.current_page = Some(page);
        self
    }
}

impl<Id> Default for WindowProps<Id> {
    fn default() -> Self {
        Self { current_page: None }
    }
}

impl<Id> Properties for WindowProps<Id> {}

pub struct Window<S, A, Id> {
    /// Internal base
    base: BaseView<S, A>,
    /// Internal properties
    props: WindowProps<Id>,
    /// All pages known
    pages: HashMap<Id, BoxedWidget<S, A>>,
}

impl<S, A, Id> Window<S, A, Id>
where
    Id: Clone + Hash + Eq + PartialEq,
{
    pub fn page(mut self, id: Id, page: BoxedWidget<S, A>) -> Self {
        self.pages.insert(id, page);
        self
    }
}

impl<'a: 'static, S, A, Id> Widget for Window<S, A, Id>
where
    Id: Clone + Hash + Eq + PartialEq + 'a,
{
    type Action = A;
    type State = S;

    fn new(_state: &S, action_tx: UnboundedSender<A>) -> Self
    where
        Self: Sized,
    {
        Self {
            base: BaseView {
                action_tx: action_tx.clone(),
                on_update: None,
                on_event: None,
            },
            props: WindowProps::default(),
            pages: HashMap::new(),
        }
    }

    fn handle_event(&mut self, key: termion::event::Key) {
        let page = self
            .props
            .current_page
            .as_ref()
            .and_then(|id| self.pages.get_mut(id));

        if let Some(page) = page {
            page.handle_event(key);
        }
    }

    fn update(&mut self, state: &S) {
        self.props =
            WindowProps::from_callback(self.base.on_update, state).unwrap_or(self.props.clone());

        let page = self
            .props
            .current_page
            .as_ref()
            .and_then(|id| self.pages.get_mut(id));

        if let Some(page) = page {
            page.update(state);
        }
    }

    fn render(&self, frame: &mut ratatui::Frame, _area: Rect, props: Option<&dyn Any>) {
        let _props = props
            .and_then(|props| props.downcast_ref::<WindowProps<Id>>())
            .unwrap_or(&self.props);

        let area = frame.size();

        let page = self
            .props
            .current_page
            .as_ref()
            .and_then(|id| self.pages.get(id));

        if let Some(page) = page {
            page.render(frame, area, None);
        }
    }

    fn base_mut(&mut self) -> &mut BaseView<S, A> {
        &mut self.base
    }
}

#[derive(Clone)]
pub struct ShortcutsProps {
    pub shortcuts: Vec<(String, String)>,
    pub divider: char,
}

impl ShortcutsProps {
    pub fn divider(mut self, divider: char) -> Self {
        self.divider = divider;
        self
    }

    pub fn shortcuts(mut self, shortcuts: &[(&str, &str)]) -> Self {
        self.shortcuts.clear();
        for (short, long) in shortcuts {
            self.shortcuts.push((short.to_string(), long.to_string()));
        }
        self
    }
}

impl Default for ShortcutsProps {
    fn default() -> Self {
        Self {
            shortcuts: vec![],
            divider: '∙',
        }
    }
}

impl Properties for ShortcutsProps {}

pub struct Shortcuts<S, A> {
    /// Internal properties
    props: ShortcutsProps,
    /// Internal base
    base: BaseView<S, A>,
}

impl<S, A> Shortcuts<S, A> {
    pub fn divider(mut self, divider: char) -> Self {
        self.props.divider = divider;
        self
    }

    pub fn shortcuts(mut self, shortcuts: &[(&str, &str)]) -> Self {
        self.props.shortcuts.clear();
        for (short, long) in shortcuts {
            self.props
                .shortcuts
                .push((short.to_string(), long.to_string()));
        }
        self
    }
}

impl<S, A> Widget for Shortcuts<S, A> {
    type Action = A;
    type State = S;

    fn new(_state: &S, action_tx: UnboundedSender<A>) -> Self {
        Self {
            base: BaseView {
                action_tx: action_tx.clone(),
                on_update: None,
                on_event: None,
            },
            props: ShortcutsProps::default(),
        }
    }

    fn handle_event(&mut self, _key: Key) {}

    fn update(&mut self, state: &S) {
        self.props =
            ShortcutsProps::from_callback(self.base.on_update, state).unwrap_or(self.props.clone());
    }

    fn render(&self, frame: &mut ratatui::Frame, area: Rect, props: Option<&dyn Any>) {
        use ratatui::widgets::Table;

        let props = props
            .and_then(|props| props.downcast_ref::<ShortcutsProps>())
            .unwrap_or(&self.props);

        let mut shortcuts = props.shortcuts.iter().peekable();
        let mut row = vec![];

        while let Some(shortcut) = shortcuts.next() {
            let short = Text::from(shortcut.0.clone()).style(style::gray());
            let long = Text::from(shortcut.1.clone()).style(style::gray().dim());
            let spacer = Text::from(String::new());
            let divider = Text::from(format!(" {} ", props.divider)).style(style::gray().dim());

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
    }

    fn base_mut(&mut self) -> &mut BaseView<S, A> {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct Column<'a> {
    pub text: Text<'a>,
    pub width: Constraint,
    pub skip: bool,
}

impl<'a> Column<'a> {
    pub fn new(text: impl Into<Text<'a>>, width: Constraint) -> Self {
        Self {
            text: text.into(),
            width,
            skip: false,
        }
    }

    pub fn skip(mut self, skip: bool) -> Self {
        self.skip = skip;
        self
    }
}

#[derive(Clone, Debug)]
pub struct TableProps<'a, R, const W: usize>
where
    R: ToRow<W>,
{
    pub items: Vec<R>,
    pub selected: Option<usize>,
    pub focus: bool,
    pub columns: Vec<Column<'a>>,
    pub has_footer: bool,
    pub cutoff: usize,
    pub cutoff_after: usize,
    pub page_size: usize,
}

impl<'a, R, const W: usize> Default for TableProps<'a, R, W>
where
    R: ToRow<W>,
{
    fn default() -> Self {
        Self {
            items: vec![],
            focus: false,
            columns: vec![],
            has_footer: false,
            cutoff: usize::MAX,
            cutoff_after: usize::MAX,
            page_size: 1,
            selected: Some(0),
        }
    }
}

impl<'a, R, const W: usize> TableProps<'a, R, W>
where
    R: ToRow<W>,
{
    pub fn items(mut self, items: Vec<R>) -> Self {
        self.items = items;
        self
    }

    pub fn selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    pub fn columns(mut self, columns: Vec<Column<'a>>) -> Self {
        self.columns = columns;
        self
    }

    pub fn footer(mut self, has_footer: bool) -> Self {
        self.has_footer = has_footer;
        self
    }

    pub fn cutoff(mut self, cutoff: usize, cutoff_after: usize) -> Self {
        self.cutoff = cutoff;
        self.cutoff_after = cutoff_after;
        self
    }

    pub fn page_size(mut self, page_size: usize) -> Self {
        self.page_size = page_size;
        self
    }
}

impl<'a: 'static, R, const W: usize> Properties for TableProps<'a, R, W> where R: ToRow<W> + 'static {}
impl WidgetState for TableState {}

pub struct Table<'a, S, A, R, const W: usize>
where
    R: ToRow<W>,
{
    /// Internal base
    base: BaseView<S, A>,
    /// Internal table properties
    props: TableProps<'a, R, W>,
    /// Internal selection and offset state
    state: TableState,
}

impl<'a, S, A, R, const W: usize> Table<'a, S, A, R, W>
where
    R: ToRow<W>,
{
    fn prev(&mut self) -> Option<usize> {
        let selected = self
            .state
            .selected()
            .map(|current| current.saturating_sub(1));
        self.state.select(selected);
        selected
    }

    fn next(&mut self, len: usize) -> Option<usize> {
        let selected = self.state.selected().map(|current| {
            if current < len.saturating_sub(1) {
                current.saturating_add(1)
            } else {
                current
            }
        });
        self.state.select(selected);
        selected
    }

    fn prev_page(&mut self, page_size: usize) -> Option<usize> {
        let selected = self
            .state
            .selected()
            .map(|current| current.saturating_sub(page_size));
        self.state.select(selected);
        selected
    }

    fn next_page(&mut self, len: usize, page_size: usize) -> Option<usize> {
        let selected = self.state.selected().map(|current| {
            if current < len.saturating_sub(1) {
                cmp::min(current.saturating_add(page_size), len.saturating_sub(1))
            } else {
                current
            }
        });
        self.state.select(selected);
        selected
    }

    fn begin(&mut self) {
        self.state.select(Some(0));
    }

    fn end(&mut self, len: usize) {
        self.state.select(Some(len.saturating_sub(1)));
    }
}

impl<'a: 'static, S, A, R, const W: usize> Widget for Table<'a, S, A, R, W>
where
    R: ToRow<W> + Clone + 'static,
{
    type Action = A;
    type State = S;

    fn new(_state: &S, action_tx: UnboundedSender<A>) -> Self {
        Self {
            base: BaseView {
                action_tx: action_tx.clone(),
                on_update: None,
                on_event: None,
            },
            props: TableProps::default(),
            state: TableState::default().with_selected(Some(0)),
        }
    }

    fn handle_event(&mut self, key: Key) {
        match key {
            Key::Up | Key::Char('k') => {
                self.prev();
            }
            Key::Down | Key::Char('j') => {
                self.next(self.props.items.len());
            }
            Key::PageUp => {
                self.prev_page(self.props.page_size);
            }
            Key::PageDown => {
                self.next_page(self.props.items.len(), self.props.page_size);
            }
            Key::Home => {
                self.begin();
            }
            Key::End => {
                self.end(self.props.items.len());
            }
            _ => {}
        }

        self.props.selected = self.state.selected();

        if let Some(on_event) = self.base.on_event {
            (on_event)(
                self.state.clone().to_boxed_any(),
                self.base.action_tx.clone(),
            );
        }
    }

    fn update(&mut self, state: &S) {
        self.props =
            TableProps::from_callback(self.base.on_update, state).unwrap_or(self.props.clone());

        // TODO: Move to state reducer
        if let Some(selected) = self.state.selected() {
            if selected > self.props.items.len() {
                self.begin();
            }
        }
    }

    fn render(&self, frame: &mut ratatui::Frame, area: Rect, props: Option<&dyn Any>) {
        let props = props
            .and_then(|props| props.downcast_ref::<TableProps<R, W>>())
            .unwrap_or(&self.props);

        let widths: Vec<Constraint> = self
            .props
            .columns
            .iter()
            .filter_map(|c| if !c.skip { Some(c.width) } else { None })
            .collect();

        let widths = if area.width < props.cutoff as u16 {
            widths.iter().take(props.cutoff_after).collect::<Vec<_>>()
        } else {
            widths.iter().collect::<Vec<_>>()
        };

        if !props.items.is_empty() {
            let rows = props
                .items
                .iter()
                .map(|item| {
                    let mut cells = vec![];
                    let mut it = props.columns.iter();

                    for cell in item.to_row() {
                        if let Some(col) = it.next() {
                            if !col.skip {
                                cells.push(cell.clone());
                            }
                        } else {
                            continue;
                        }
                    }

                    Row::new(cells)
                })
                .collect::<Vec<_>>();
            let rows = ratatui::widgets::Table::default()
                .rows(rows)
                .widths(widths)
                .column_spacing(1)
                .highlight_style(style::highlight());

            frame.render_stateful_widget(rows, area, &mut self.state.clone());
        } else {
            let center = layout::centered_rect(area, 50, 10);
            let hint = Text::from(span::default("Nothing to show"))
                .centered()
                .light_magenta()
                .dim();

            frame.render_widget(hint, center);
        }
    }

    fn base_mut(&mut self) -> &mut BaseView<S, A> {
        &mut self.base
    }
}

pub struct TableUtils {}

impl TableUtils {
    pub fn progress(selected: usize, len: usize, page_size: usize) -> usize {
        let step = selected;
        let page_size = page_size as f64;
        let len = len as f64;

        let lines = page_size + step.saturating_sub(page_size as usize) as f64;
        let progress = (lines / len * 100.0).ceil();

        if progress > 97.0 {
            Self::map_range((0.0, progress), (0.0, 100.0), progress) as usize
        } else {
            progress as usize
        }
    }

    fn map_range(from: (f64, f64), to: (f64, f64), value: f64) -> f64 {
        to.0 + (value - from.0) * (to.1 - to.0) / (from.1 - from.0)
    }
}
