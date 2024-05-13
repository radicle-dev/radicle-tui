use std::cmp;

use ratatui::widgets::Row;
use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::layout::Constraint;
use ratatui::style::Stylize;
use ratatui::text::Text;
use ratatui::widgets::TableState;

use crate::ui::theme::style;
use crate::ui::{layout, span};

use super::{container::Column, BaseView, Properties, RenderProps, ToRow, Widget, WidgetState};

#[derive(Clone, Debug)]
pub struct TableProps<'a, R, const W: usize>
where
    R: ToRow<W>,
{
    pub items: Vec<R>,
    pub selected: Option<usize>,
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

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let widths: Vec<Constraint> = self
            .props
            .columns
            .iter()
            .filter_map(|c| if !c.skip { Some(c.width) } else { None })
            .collect();

        let widths = if props.area.width < self.props.cutoff as u16 {
            widths
                .iter()
                .take(self.props.cutoff_after)
                .collect::<Vec<_>>()
        } else {
            widths.iter().collect::<Vec<_>>()
        };

        if !self.props.items.is_empty() {
            let rows = self
                .props
                .items
                .iter()
                .map(|item| {
                    let mut cells = vec![];
                    let mut it = self.props.columns.iter();

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

            frame.render_stateful_widget(rows, props.area, &mut self.state.clone());
        } else {
            let center = layout::centered_rect(props.area, 50, 10);
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
