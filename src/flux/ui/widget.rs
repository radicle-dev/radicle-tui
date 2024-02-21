use std::fmt::Debug;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, Cell, Row, TableState};

use super::ext::{FooterBlock, HeaderBlock};
use super::theme::style;

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
pub struct FooterProps<'a> {
    pub cells: Vec<Text<'a>>,
    pub widths: Vec<Constraint>,
}

#[derive(Debug)]
pub struct TableProps<'a, R: ToRow<W>, const W: usize> {
    pub items: Vec<R>,
    pub focus: bool,
    pub widths: [Constraint; W],
    pub header: [Cell<'a>; W],
    pub footer: Option<FooterProps<'a>>,
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

    pub fn prev(&mut self) {
        let selected = self.selected().map(|current| current.saturating_sub(1));
        self.state.select(selected);
    }

    pub fn next(&mut self, len: usize) {
        let selected = self.selected().map(|current| {
            if current < len.saturating_sub(1) {
                current.saturating_add(1)
            } else {
                current
            }
        });
        self.state.select(selected);
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

impl<'a, A, R, const W: usize> Render<TableProps<'a, R, W>> for Table<A>
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

        let layout = if props.footer.is_some() {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints(vec![
                    Constraint::Length(3),
                    Constraint::Min(1),
                    Constraint::Length(3),
                ])
                .split(area)
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints(vec![Constraint::Length(3), Constraint::Min(1)])
                .split(area)
        };

        // Render header
        let block = HeaderBlock::default()
            .borders(Borders::ALL)
            .border_style(style::border(props.focus))
            .border_type(BorderType::Rounded);

        let header_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Min(1)])
            .vertical_margin(1)
            .horizontal_margin(1)
            .split(layout[0]);

        let header = Row::new(props.header).style(style::reset().bold());
        let header = ratatui::widgets::Table::default()
            .column_spacing(1)
            .header(header)
            .widths(widths.clone());

        frame.render_widget(block, layout[0]);
        frame.render_widget(header, header_layout[0]);

        // Render content
        let table_borders = if props.footer.is_some() {
            Borders::LEFT | Borders::RIGHT
        } else {
            Borders::BOTTOM | Borders::LEFT | Borders::RIGHT
        };

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
                    .borders(table_borders),
            )
            .highlight_style(style::highlight());

        frame.render_stateful_widget(rows, layout[1], &mut self.state.clone());

        if let Some(footer) = props.footer {
            // Render footer
            let footer_block = FooterBlock::default()
                .borders(Borders::ALL)
                .border_style(style::border(props.focus))
                .border_type(BorderType::Rounded);

            let footer_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints(vec![Constraint::Min(1)])
                .vertical_margin(1)
                .horizontal_margin(1)
                .split(layout[2]);

            let footer = ratatui::widgets::Table::default()
                .column_spacing(1)
                .header(Row::new(footer.cells))
                .widths(footer.widths);

            frame.render_widget(footer_block, layout[2]);
            frame.render_widget(footer, footer_layout[0]);
        }
    }
}
