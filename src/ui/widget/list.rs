use std::cmp;
use std::marker::PhantomData;

use ratatui::widgets::{Cell, Row};
use ratatui::Frame;
use termion::event::Key;

use ratatui::layout::Constraint;
use ratatui::style::Stylize;
use ratatui::text::Text;
use ratatui::widgets::TableState;

use crate::ui::theme::style;
use crate::ui::{layout, span};

use super::{container::Column, RenderProps, View};
use super::{ViewProps, ViewState};

/// Needs to be implemented for items that are supposed to be rendered in tables.
pub trait ToRow<const W: usize> {
    fn to_row(&self) -> [Cell; W];
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
    pub page_size: usize,
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

    pub fn page_size(mut self, page_size: usize) -> Self {
        self.page_size = page_size;
        self
    }
}

pub struct Table<S, M, R, const W: usize>
where
    R: ToRow<W>,
{
    /// Internal selection and offset state
    state: TableState,
    /// Phantom
    phantom: PhantomData<(S, M, R)>,
}

impl<S, M, R, const W: usize> Default for Table<S, M, R, W>
where
    R: ToRow<W>,
{
    fn default() -> Self {
        Self {
            state: TableState::default().with_selected(Some(0)),
            phantom: PhantomData,
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

        match key {
            Key::Up | Key::Char('k') => {
                self.prev();
            }
            Key::Down | Key::Char('j') => {
                self.next(props.items.len());
            }
            Key::PageUp => {
                self.prev_page(props.page_size);
            }
            Key::PageDown => {
                self.next_page(props.items.len(), props.page_size);
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

        if props.selected != self.state.selected() {
            self.state.select(props.selected);
        }
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

            frame.render_stateful_widget(rows, render.area, &mut self.state);
        } else {
            let center = layout::centered_rect(render.area, 50, 10);
            let hint = Text::from(span::default("Nothing to show"))
                .centered()
                .light_magenta()
                .dim();

            frame.render_widget(hint, center);
        }
    }

    fn view_state(&self) -> Option<ViewState> {
        self.state.selected().map(ViewState::USize)
    }
}
