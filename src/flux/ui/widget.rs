pub mod container;

use std::cmp;
use std::fmt::Debug;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, Cell, Row, TableState};

use super::theme::style;
use super::{layout, span};

pub trait Widget<S, A> {
    fn new(state: &S, action_tx: UnboundedSender<A>) -> Self
    where
        Self: Sized;

    fn move_with_state(self, state: &S) -> Self
    where
        Self: Sized;

    fn name(&self) -> &str;

    fn handle_key_event(&mut self, key: Key);
}

pub trait Render<P> {
    fn render<B: ratatui::backend::Backend>(&self, frame: &mut Frame, area: Rect, props: P);
}

pub struct Shortcut {
    pub short: String,
    pub long: String,
}

impl Shortcut {
    pub fn new(short: &str, long: &str) -> Self {
        Self {
            short: short.to_string(),
            long: long.to_string(),
        }
    }
}

pub struct ShortcutsProps {
    pub shortcuts: Vec<Shortcut>,
    pub divider: char,
}

pub struct Shortcuts<A> {
    pub action_tx: UnboundedSender<A>,
}

impl<S, A> Widget<S, A> for Shortcuts<A> {
    fn new(state: &S, action_tx: UnboundedSender<A>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, _state: &S) -> Self
    where
        Self: Sized,
    {
        Self { ..self }
    }

    fn name(&self) -> &str {
        "shortcuts"
    }

    fn handle_key_event(&mut self, _key: termion::event::Key) {}
}

impl<A> Render<ShortcutsProps> for Shortcuts<A> {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, props: ShortcutsProps) {
        use ratatui::widgets::Table;

        let mut shortcuts = props.shortcuts.iter().peekable();
        let mut row = vec![];

        while let Some(shortcut) = shortcuts.next() {
            let short = Text::from(shortcut.short.clone()).style(style::gray());
            let long = Text::from(shortcut.long.clone()).style(style::gray().dim());
            let spacer = Text::from(String::new());
            let divider = Text::from(format!(" {} ", props.divider)).style(style::gray().dim());

            row.push((shortcut.short.chars().count(), short));
            row.push((1, spacer));
            row.push((shortcut.long.chars().count(), long));

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
}

pub trait ToRow<const W: usize> {
    fn to_row(&self) -> [Cell; W];
}

#[derive(Debug)]
pub struct TableProps<R: ToRow<W>, const W: usize> {
    pub items: Vec<R>,
    pub focus: bool,
    pub widths: [Constraint; W],
    pub has_header: bool,
    pub has_footer: bool,
    pub cutoff: usize,
    pub cutoff_after: usize,
}

pub struct Table<A> {
    /// Sending actions to the state store
    pub action_tx: UnboundedSender<A>,
    /// Internal selection state
    state: TableState,
}

impl<A> Table<A> {
    pub fn selected(&self) -> Option<usize> {
        self.state.selected()
    }

    pub fn prev(&mut self) -> Option<usize> {
        let selected = self.selected().map(|current| current.saturating_sub(1));
        self.state.select(selected);
        selected
    }

    pub fn next(&mut self, len: usize) -> Option<usize> {
        let selected = self.selected().map(|current| {
            if current < len.saturating_sub(1) {
                current.saturating_add(1)
            } else {
                current
            }
        });
        self.state.select(selected);
        selected
    }

    pub fn prev_page(&mut self, page_size: usize) -> Option<usize> {
        let selected = self
            .selected()
            .map(|current| current.saturating_sub(page_size));
        self.state.select(selected);
        selected
    }

    pub fn next_page(&mut self, len: usize, page_size: usize) -> Option<usize> {
        let selected = self.selected().map(|current| {
            if current < len.saturating_sub(1) {
                cmp::min(current.saturating_add(page_size), len.saturating_sub(1))
            } else {
                current
            }
        });
        self.state.select(selected);
        selected
    }

    pub fn begin(&mut self) -> Option<usize> {
        self.state.select(Some(0));
        self.state.selected()
    }

    pub fn end(&mut self, len: usize) -> Option<usize> {
        self.state.select(Some(len.saturating_sub(1)));
        self.state.selected()
    }

    pub fn progress(&self, len: usize) -> (usize, usize) {
        let step = self
            .selected()
            .map(|selected| selected.saturating_add(1))
            .unwrap_or_default();

        (cmp::min(step, len), len)
    }

    pub fn progress_percentage(&self, len: usize, page_size: usize) -> usize {
        let step = self.selected().unwrap_or_default();
        let page_size = page_size as f64;
        let len = len as f64;

        let lines = page_size + step.saturating_sub(page_size as usize) as f64;
        let progress = (lines / len * 100_f64).ceil() as usize;

        if progress > 97 {
            Self::map_range((0, progress), (0, 100), progress)
        } else {
            progress
        }
    }

    fn map_range(from: (usize, usize), to: (usize, usize), value: usize) -> usize {
        to.0 + (value - from.0) * (to.1 - to.0) / (from.1 - from.0)
    }
}

impl<S, A> Widget<S, A> for Table<A> {
    fn new(state: &S, action_tx: UnboundedSender<A>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
            state: TableState::default().with_selected(Some(0)),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, _state: &S) -> Self
    where
        Self: Sized,
    {
        Self { ..self }
    }

    fn name(&self) -> &str {
        "shortcuts"
    }

    fn handle_key_event(&mut self, _key: Key) {}
}

impl<A, R, const W: usize> Render<TableProps<R, W>> for Table<A>
where
    R: ToRow<W> + Debug,
{
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, props: TableProps<R, W>) {
        let widths = props.widths.to_vec();
        let widths = if area.width < props.cutoff as u16 {
            widths.iter().take(props.cutoff_after).collect::<Vec<_>>()
        } else {
            widths.iter().collect::<Vec<_>>()
        };

        let borders = match (props.has_header, props.has_footer) {
            (false, false) => Borders::ALL,
            (true, false) => Borders::BOTTOM | Borders::LEFT | Borders::RIGHT,
            (false, true) => Borders::TOP | Borders::LEFT | Borders::RIGHT,
            (true, true) => Borders::LEFT | Borders::RIGHT,
        };

        if !props.items.is_empty() {
            let rows = props
                .items
                .iter()
                .map(|item| Row::new(item.to_row()))
                .collect::<Vec<_>>();
            let rows = ratatui::widgets::Table::default()
                .rows(rows)
                .widths(widths)
                .column_spacing(1)
                .block(
                    Block::default()
                        .border_style(style::border(props.focus))
                        .border_type(BorderType::Rounded)
                        .borders(borders),
                )
                .highlight_style(style::highlight());

            frame.render_stateful_widget(rows, area, &mut self.state.clone());
        } else {
            let block = Block::default()
                .border_style(style::border(props.focus))
                .border_type(BorderType::Rounded)
                .borders(borders);

            frame.render_widget(block, area);

            let center = layout::centered_rect(area, 50, 10);
            let hint = Text::from(span::default("Nothing to show".to_string()))
                .centered()
                .light_magenta()
                .dim();

            frame.render_widget(hint, center);
        }
    }
}
