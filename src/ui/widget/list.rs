use std::collections::HashSet;
use std::hash::Hash;
use std::marker::PhantomData;
use std::{cmp, vec};

use termion::event::Key;

use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Style, Stylize};
use ratatui::symbols::border;
use ratatui::text::Text;
use ratatui::widgets::TableState;
use ratatui::widgets::{
    Block, Borders, Cell, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
};
use ratatui::Frame;

use tui_tree_widget::{TreeItem, TreeState};

use crate::ui::theme::style;
use crate::ui::{layout, span};

use super::{container::Column, RenderProps, View};
use super::{utils, ViewProps, ViewState};

/// Needs to be implemented for items that are supposed to be rendered in tables.
pub trait ToRow<const W: usize> {
    fn to_row(&self) -> [Cell; W];
}

/// Needs to be implemented for items that are supposed to be rendered in trees.
pub trait ToTree<Id>
where
    Id: ToString,
{
    fn rows(&self) -> Vec<TreeItem<'_, Id>>;
}

#[derive(Clone, Debug)]
pub struct TableProps<'a, R, const W: usize>
where
    R: ToRow<W>,
{
    pub items: Vec<R>,
    pub selected: Option<usize>,
    pub columns: Vec<Column<'a>>,
    pub show_scrollbar: bool,
    pub dim: bool,
}

impl<'a, R, const W: usize> Default for TableProps<'a, R, W>
where
    R: ToRow<W>,
{
    fn default() -> Self {
        Self {
            items: vec![],
            columns: vec![],
            show_scrollbar: true,
            selected: Some(0),
            dim: false,
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

    pub fn show_scrollbar(mut self, show_scrollbar: bool) -> Self {
        self.show_scrollbar = show_scrollbar;
        self
    }

    pub fn dim(mut self, dim: bool) -> Self {
        self.dim = dim;
        self
    }
}

pub struct Table<S, M, R, const W: usize>
where
    R: ToRow<W>,
{
    /// Internal selection and offset state
    state: (TableState, usize),
    /// Phantom
    phantom: PhantomData<(S, M, R)>,
    /// Current render height
    height: u16,
}

impl<S, M, R, const W: usize> Default for Table<S, M, R, W>
where
    R: ToRow<W>,
{
    fn default() -> Self {
        Self {
            state: (TableState::default().with_selected(Some(0)), 0),
            phantom: PhantomData,
            height: 1,
        }
    }
}

impl<S, M, R, const W: usize> Table<S, M, R, W>
where
    R: ToRow<W>,
{
    fn prev(&mut self) -> Option<usize> {
        let selected = self
            .state
            .0
            .selected()
            .map(|current| current.saturating_sub(1));
        self.state.0.select(selected);
        selected
    }

    fn next(&mut self, len: usize) -> Option<usize> {
        let selected = self.state.0.selected().map(|current| {
            if current < len.saturating_sub(1) {
                current.saturating_add(1)
            } else {
                current
            }
        });
        self.state.0.select(selected);
        selected
    }

    fn prev_page(&mut self, page_size: usize) -> Option<usize> {
        let selected = self
            .state
            .0
            .selected()
            .map(|current| current.saturating_sub(page_size));
        self.state.0.select(selected);
        selected
    }

    fn next_page(&mut self, len: usize, page_size: usize) -> Option<usize> {
        let selected = self.state.0.selected().map(|current| {
            if current < len.saturating_sub(1) {
                cmp::min(current.saturating_add(page_size), len.saturating_sub(1))
            } else {
                current
            }
        });
        self.state.0.select(selected);
        selected
    }

    fn begin(&mut self) {
        self.state.0.select(Some(0));
    }

    fn end(&mut self, len: usize) {
        self.state.0.select(Some(len.saturating_sub(1)));
    }
}

impl<S, M, R, const W: usize> View for Table<S, M, R, W>
where
    S: 'static,
    M: 'static,
    R: ToRow<W> + Clone + 'static,
{
    type Message = M;
    type State = S;

    fn handle_event(&mut self, props: Option<&ViewProps>, key: Key) -> Option<Self::Message> {
        let default = TableProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<TableProps<R, W>>())
            .unwrap_or(&default);

        let page_size = self.height;

        match key {
            Key::Up | Key::Char('k') => {
                self.prev();
            }
            Key::Down | Key::Char('j') => {
                self.next(props.items.len());
            }
            Key::PageUp => {
                self.prev_page(page_size as usize);
            }
            Key::PageDown => {
                self.next_page(props.items.len(), page_size as usize);
            }
            Key::Home => {
                self.begin();
            }
            Key::End => {
                self.end(props.items.len());
            }
            _ => {}
        }

        None
    }

    fn update(&mut self, props: Option<&ViewProps>, _state: &Self::State) {
        let default = TableProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<TableProps<R, W>>())
            .unwrap_or(&default);

        if props.selected != self.state.0.selected() {
            self.state.0.select(props.selected);
        }
        self.state.1 = props.items.len();
    }

    fn render(&mut self, props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let default = TableProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<TableProps<R, W>>())
            .unwrap_or(&default);

        let show_scrollbar = props.show_scrollbar && props.items.len() >= self.height.into();
        let has_items = !props.items.is_empty();

        let widths: Vec<Constraint> = props
            .columns
            .iter()
            .filter_map(|c| {
                if !c.skip && c.displayed(render.area.width as usize) {
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
            .areas(render.area);

            let rows = props
                .items
                .iter()
                .map(|item| {
                    let mut cells = vec![];
                    let mut it = props.columns.iter();

                    for cell in item.to_row() {
                        if let Some(col) = it.next() {
                            if !col.skip && col.displayed(render.area.width as usize) {
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
                .highlight_style(style::highlight(render.focus));

            let table = if !render.focus && props.dim {
                table.dim()
            } else {
                table
            };

            frame.render_stateful_widget(table, table_area, &mut self.state.0);

            let scroller = Scrollbar::default()
                .begin_symbol(None)
                .track_symbol(None)
                .end_symbol(None)
                .thumb_symbol("┃")
                .style(if render.focus {
                    Style::default()
                } else {
                    Style::default().dim()
                });
            let mut scroller_state = ScrollbarState::default()
                .content_length(props.items.len().saturating_sub(self.height.into()))
                .position(self.state.0.offset());
            frame.render_stateful_widget(scroller, scroller_area, &mut scroller_state);
        } else {
            let center = layout::centered_rect(render.area, 50, 10);
            let hint = Text::from(span::default("Nothing to show"))
                .centered()
                .light_magenta()
                .dim();

            frame.render_widget(hint, center);
        }

        self.height = render.area.height;
    }

    fn view_state(&self) -> Option<ViewState> {
        let selected = self.state.0.selected().unwrap_or_default();

        Some(ViewState::Table {
            selected,
            scroll: utils::scroll::percent_absolute(
                selected.saturating_sub(self.height.into()),
                self.state.1,
                self.height.into(),
            ),
        })
    }
}

#[derive(Clone, Debug)]
pub struct TreeProps<R, Id>
where
    R: ToTree<Id> + Clone,
    Id: ToString,
{
    /// Root items.
    pub items: Vec<R>,
    /// Optional path to selected item, e.g. ["1.0", "1.0.1", "1.0.2"]. If not `None`,
    /// it will override the internal tree state.
    pub selected: Option<Vec<Id>>,
    /// If this widget should render its scrollbar. Default: `true`.
    pub show_scrollbar: bool,
    /// Optional identifier set of opened items. If not `None`,
    /// it will override the internal tree state.
    pub opened: Option<HashSet<Vec<Id>>>,
    /// Set to `true` if the content style should be dimmed whenever the widget
    /// has no focus.
    pub dim: bool,
}

impl<R, Id> Default for TreeProps<R, Id>
where
    R: ToTree<Id> + Clone,
    Id: ToString,
{
    fn default() -> Self {
        Self {
            items: vec![],
            selected: None,
            show_scrollbar: true,
            opened: None,
            dim: false,
        }
    }
}

impl<R, Id> TreeProps<R, Id>
where
    R: ToTree<Id> + Clone,
    Id: ToString + Clone,
{
    pub fn items(mut self, items: Vec<R>) -> Self {
        self.items = items;
        self
    }

    pub fn selected(mut self, selected: Option<&[Id]>) -> Self {
        self.selected = selected.map(|s| s.to_vec());
        self
    }

    pub fn opened(mut self, opened: Option<HashSet<Vec<Id>>>) -> Self {
        self.opened = opened;
        self
    }

    pub fn show_scrollbar(mut self, show_scrollbar: bool) -> Self {
        self.show_scrollbar = show_scrollbar;
        self
    }

    pub fn dim(mut self, dim: bool) -> Self {
        self.dim = dim;
        self
    }
}

/// A `Tree` is an expandable, collapsable and scrollable tree widget, that takes
/// a list of root items which implement `ToTree`. It can be updated with a selection
/// and a set of opened items.
pub struct Tree<S, M, R, Id>
where
    R: ToTree<Id>,
    Id: ToString + Clone,
{
    /// Internal selection and offset state
    state: TreeState<Id>,
    /// Phantom
    phantom: PhantomData<(S, M, R, Id)>,
}

impl<S, M, R, Id> Default for Tree<S, M, R, Id>
where
    R: ToTree<Id>,
    Id: ToString + Clone + Default,
{
    fn default() -> Self {
        Self {
            state: TreeState::default(),
            phantom: PhantomData,
        }
    }
}

impl<S, M, R, Id> View for Tree<S, M, R, Id>
where
    R: ToTree<Id> + Clone + 'static,
    Id: ToString + Clone + Default + Eq + PartialEq + Hash + 'static,
{
    type State = S;
    type Message = M;

    fn reset(&mut self) {
        self.state = TreeState::default();
    }

    fn update(&mut self, props: Option<&ViewProps>, _state: &Self::State) {
        let default = TreeProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<TreeProps<R, Id>>())
            .unwrap_or(&default);

        if let Some(selected) = &props.selected {
            if selected != self.state.selected() {
                self.state.select(selected.clone());
            }
        }

        if let Some(opened) = &props.opened {
            if opened != self.state.opened() {
                self.state.close_all();
                for path in opened {
                    self.state.open(path.to_vec());
                }
            }
        }
    }

    fn handle_event(&mut self, _props: Option<&ViewProps>, key: Key) -> Option<Self::Message> {
        match key {
            Key::Up | Key::Char('k') => {
                self.state.key_up();
            }
            Key::Down | Key::Char('j') => {
                self.state.key_down();
            }
            Key::Left | Key::Char('h')
                if !self.state.selected().is_empty() && !self.state.opened().is_empty() =>
            {
                self.state.key_left();
            }
            Key::Right | Key::Char('l') => {
                self.state.key_right();
            }
            _ => {}
        }

        None
    }

    fn render(&mut self, props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let default = TreeProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<TreeProps<R, Id>>())
            .unwrap_or(&default);

        let mut items = vec![];
        for item in &props.items {
            items.extend(item.rows());
        }

        let tree_style = if !render.focus && props.dim {
            Style::default().dim()
        } else {
            Style::default()
        };

        let tree = if props.show_scrollbar {
            tui_tree_widget::Tree::new(&items)
                .expect("all item identifiers are unique")
                .block(
                    Block::default()
                        .borders(Borders::RIGHT)
                        .border_set(border::Set {
                            vertical_right: " ",
                            ..Default::default()
                        })
                        .border_style(if render.focus {
                            Style::default()
                        } else {
                            Style::default().dim()
                        }),
                )
                .experimental_scrollbar(Some(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .begin_symbol(None)
                        .track_symbol(None)
                        .end_symbol(None)
                        .thumb_symbol("┃"),
                ))
                .highlight_style(style::highlight(render.focus))
                .style(tree_style)
        } else {
            tui_tree_widget::Tree::new(&items)
                .expect("all item identifiers are unique")
                .style(tree_style)
                .highlight_style(style::highlight(render.focus))
        };

        frame.render_stateful_widget(tree, render.area, &mut self.state);
    }

    fn view_state(&self) -> Option<ViewState> {
        Some(ViewState::Tree(
            self.state
                .selected()
                .to_vec()
                .iter()
                .map(|s| s.to_string())
                .collect(),
        ))
    }
}
