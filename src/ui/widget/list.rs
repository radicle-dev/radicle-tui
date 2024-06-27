use std::marker::PhantomData;
use std::{cmp, vec};

use ratatui::symbols::{self, border};
use ratatui::widgets::{Block, Borders, Cell, Row, Scrollbar, ScrollbarOrientation};
use ratatui::Frame;
use termion::event::Key;

use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::TableState;
use tui_tree_widget::{TreeItem, TreeState};

use crate::ui::theme::style;
use crate::ui::{layout, span};

use super::{container::Column, RenderProps, View};
use super::{utils, ViewProps, ViewState};

/// Needs to be implemented for items that are supposed to be rendered in tables.
pub trait ToRow<const W: usize> {
    fn to_row(&self) -> [Cell; W];
}

/// Needs to be implemented for items that are supposed to be rendered in tables.
pub trait ToTree {
    fn rows<'a>(&'a self) -> Vec<TreeItem<'a, String>>;
}

#[derive(Clone, Debug)]
pub struct TableProps<'a, R, const W: usize>
where
    R: ToRow<W>,
{
    pub items: Vec<R>,
    pub selected: Option<usize>,
    pub columns: Vec<Column<'a>>,
    pub has_footer: bool,
}

impl<'a, R, const W: usize> Default for TableProps<'a, R, W>
where
    R: ToRow<W>,
{
    fn default() -> Self {
        Self {
            items: vec![],
            columns: vec![],
            has_footer: false,
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

        if !props.items.is_empty() {
            let rows = props
                .items
                .iter()
                .map(|item| {
                    let mut cells = vec![];
                    let mut it = props.columns.iter();

                    for cell in item.to_row() {
                        if let Some(col) = it.next() {
                            if !col.skip && col.displayed(render.area.width as usize) {
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
                .highlight_style(style::highlight(render.focus));

            frame.render_stateful_widget(rows, render.area, &mut self.state.0);
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
pub struct TreeProps<R>
where
    R: ToTree + Clone,
{
    /// Root items of this tree.
    pub items: Vec<R>,
    /// Path of currently selected item, e.g. ["1.0", "1.1", "1.1.3"]
    pub selected: Vec<String>,
    /// If this widget should render its scroll progress. Default: `false`.
    show_scroll_progress: bool,
}

impl<R> Default for TreeProps<R>
where
    R: ToTree + Clone,
{
    fn default() -> Self {
        Self {
            items: vec![],
            selected: vec![],
            show_scroll_progress: false,
        }
    }
}

impl<R> TreeProps<R>
where
    R: ToTree + Clone,
{
    pub fn items(mut self, items: Vec<R>) -> Self {
        self.items = items;
        self
    }

    pub fn selected(mut self, selected: &[String]) -> Self {
        self.selected = selected.to_vec();
        self
    }

    pub fn show_scroll_progress(mut self, show_scroll_progress: bool) -> Self {
        self.show_scroll_progress = show_scroll_progress;
        self
    }
}

pub struct Tree<S, M, R>
where
    R: ToTree,
{
    /// Internal selection and offset state
    state: TreeState<String>,
    /// Phantom
    phantom: PhantomData<(S, M, R)>,
}

impl<S, M, R> Default for Tree<S, M, R>
where
    R: ToTree,
{
    fn default() -> Self {
        Self {
            state: TreeState::default(),
            phantom: PhantomData,
        }
    }
}

impl<S, M, R> View for Tree<S, M, R>
where
    R: ToTree + Clone + 'static,
{
    type State = S;
    type Message = M;

    fn reset(&mut self) {
        self.state = TreeState::default();
    }

    fn update(&mut self, props: Option<&ViewProps>, _state: &Self::State) {
        let default = TreeProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<TreeProps<R>>())
            .unwrap_or(&default);

        // if props.selected != self.state.selected() {
        //     self.state.select(props.selected.clone());
        // }
    }

    fn handle_event(&mut self, _props: Option<&ViewProps>, key: Key) -> Option<Self::Message> {
        match key {
            Key::Up | Key::Char('k') => {
                self.state.key_up();
            }
            Key::Down | Key::Char('j') => {
                self.state.key_down();
            }
            Key::Left | Key::Char('h') => {
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
            .and_then(|props| props.inner_ref::<TreeProps<R>>())
            .unwrap_or(&default);

        let [area] = Layout::default()
            .constraints([Constraint::Min(1)])
            .horizontal_margin(1)
            .areas(render.area);

        let [content_area, progress_area] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(if props.show_scroll_progress { 1 } else { 0 }),
        ])
        .areas(area);

        // let mut items = vec![];
        // for item in &props.items {
        //     items.extend(item.rows());
        // }

        let items = vec![
            TreeItem::new_leaf("a".to_string(), "Alfa"),
            TreeItem::new(
                "b".to_string(),
                "Bravo",
                vec![
                    TreeItem::new_leaf("c".to_string(), "Charlie"),
                    TreeItem::new(
                        "d".to_string(),
                        "Delta",
                        vec![
                            TreeItem::new_leaf("e".to_string(), "Echo"),
                            TreeItem::new_leaf("f".to_string(), "Foxtrot"),
                        ],
                    )
                    .expect("all item identifiers are unique"),
                    TreeItem::new_leaf("g".to_string(), "Golf"),
                ],
            )
            .expect("all item identifiers are unique"),
            TreeItem::new_leaf("h".to_string(), "Hotel"),
            TreeItem::new(
                "i".to_string(),
                "India",
                vec![
                    TreeItem::new_leaf("j".to_string(), "Juliett"),
                    TreeItem::new_leaf("k".to_string(), "Kilo"),
                    TreeItem::new_leaf("l".to_string(), "Lima"),
                    TreeItem::new_leaf("m".to_string(), "Mike"),
                    TreeItem::new_leaf("n".to_string(), "November"),
                ],
            )
            .expect("all item identifiers are unique"),
            TreeItem::new_leaf("o".to_string(), "Oscar"),
            TreeItem::new(
                "p".to_string(),
                "Papa",
                vec![
                    TreeItem::new_leaf("q".to_string(), "Quebec"),
                    TreeItem::new_leaf("r".to_string(), "Romeo"),
                    TreeItem::new_leaf("s".to_string(), "Sierra"),
                    TreeItem::new_leaf("t".to_string(), "Tango"),
                    TreeItem::new_leaf("u".to_string(), "Uniform"),
                    TreeItem::new(
                        "v".to_string(),
                        "Victor",
                        vec![
                            TreeItem::new_leaf("w".to_string(), "Whiskey"),
                            TreeItem::new_leaf("x".to_string(), "Xray"),
                            TreeItem::new_leaf("y".to_string(), "Yankee"),
                        ],
                    )
                    .expect("all item identifiers are unique"),
                ],
            )
            .expect("all item identifiers are unique"),
            TreeItem::new_leaf("z".to_string(), "Zulu"),
        ];

        let scroll_progress = utils::scroll::percent_absolute(
            self.state.get_offset(),
            items.len(),
            content_area.height.into(),
        );

        let tree = tui_tree_widget::Tree::new(&items)
            .expect("all item identifiers are unique")
            .block(
                Block::default()
                    .borders(Borders::RIGHT)
                    // .border_set(border::Set {
                    //     top_left: "",
                    //     top_right: "",
                    //     bottom_left: "",
                    //     bottom_right: "",
                    //     vertical_left: "",
                    //     vertical_right: "",
                    //     horizontal_top: "",
                    //     horizontal_bottom: "",
                    // }),
                    .border_style(style::border(false)),
            )
            .experimental_scrollbar(Some(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .track_symbol(None)
                    .end_symbol(None),
            ))
            .highlight_style(style::highlight(render.focus));

        let progress_info = if props.show_scroll_progress && scroll_progress > 0 {
            vec![Span::styled(
                format!("{}%", scroll_progress),
                Style::default().dim(),
            )]
        } else {
            vec![]
        };

        frame.render_stateful_widget(tree, render.area, &mut self.state);
        // frame.render_widget(
        //     Line::from(progress_info).alignment(Alignment::Right),
        //     progress_area,
        // );
    }

    fn view_state(&self) -> Option<ViewState> {
        Some(ViewState::Tree(self.state.selected().to_vec()))
    }
}
