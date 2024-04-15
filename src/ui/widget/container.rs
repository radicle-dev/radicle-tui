use std::fmt::Debug;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::prelude::*;
use ratatui::widgets::{BorderType, Borders, Row};

use crate::ui::ext::{FooterBlock, FooterBlockType, HeaderBlock};
use crate::ui::theme::style;

use super::{Column, Render, View};

#[derive(Debug)]
pub struct HeaderProps<'a> {
    pub columns: Vec<Column<'a>>,
    pub cutoff: usize,
    pub cutoff_after: usize,
    pub focus: bool,
}

impl<'a> Default for HeaderProps<'a> {
    fn default() -> Self {
        Self {
            columns: vec![],
            cutoff: usize::MAX,
            cutoff_after: usize::MAX,
            focus: false,
        }
    }
}

pub struct Header<'a, A> {
    /// Sending actions to the state store
    pub action_tx: UnboundedSender<A>,
    /// Internal props
    props: HeaderProps<'a>,
}

impl<'a, A> Header<'a, A> {
    pub fn columns(mut self, columns: Vec<Column<'a>>) -> Self {
        self.props.columns = columns;
        self
    }

    pub fn focus(mut self, focus: bool) -> Self {
        self.props.focus = focus;
        self
    }

    pub fn cutoff(mut self, cutoff: usize, cutoff_after: usize) -> Self {
        self.props.cutoff = cutoff;
        self.props.cutoff_after = cutoff_after;
        self
    }
}

impl<'a, A> View<(), A> for Header<'a, A> {
    fn new(state: &(), action_tx: UnboundedSender<A>) -> Self {
        Self {
            action_tx: action_tx.clone(),
            props: HeaderProps::default(),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, _state: &()) -> Self {
        Self { ..self }
    }

    fn handle_key_event(&mut self, _key: Key) {}
}

impl<'a, A, B: Backend> Render<B, ()> for Header<'a, A> {
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let widths: Vec<Constraint> = self
            .props
            .columns
            .iter()
            .filter_map(|column| {
                if !column.skip {
                    Some(column.width)
                } else {
                    None
                }
            })
            .collect();
        let cells = self
            .props
            .columns
            .iter()
            .filter_map(|column| {
                if !column.skip {
                    Some(column.text.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let widths = if area.width < self.props.cutoff as u16 {
            widths
                .iter()
                .take(self.props.cutoff_after)
                .collect::<Vec<_>>()
        } else {
            widths.iter().collect::<Vec<_>>()
        };

        // Render header
        let block = HeaderBlock::default()
            .borders(Borders::ALL)
            .border_style(style::border(self.props.focus))
            .border_type(BorderType::Rounded);

        let header_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Min(1)])
            .vertical_margin(1)
            .horizontal_margin(1)
            .split(area);

        let header = Row::new(cells).style(style::reset().bold());
        let header = ratatui::widgets::Table::default()
            .column_spacing(1)
            .header(header)
            .widths(widths.clone());

        frame.render_widget(block, area);
        frame.render_widget(header, header_layout[0]);
    }
}

#[derive(Debug)]
pub struct FooterProps<'a> {
    pub columns: Vec<Column<'a>>,
    pub cutoff: usize,
    pub cutoff_after: usize,
    pub focus: bool,
}

impl<'a> Default for FooterProps<'a> {
    fn default() -> Self {
        Self {
            columns: vec![],
            cutoff: usize::MAX,
            cutoff_after: usize::MAX,
            focus: false,
        }
    }
}

pub struct Footer<'a, A> {
    /// Message sender
    pub action_tx: UnboundedSender<A>,
    /// Internal properties
    props: FooterProps<'a>,
}

impl<'a, A> Footer<'a, A> {
    pub fn columns(mut self, columns: Vec<Column<'a>>) -> Self {
        self.props.columns = columns;
        self
    }

    pub fn cutoff(mut self, cutoff: usize, cutoff_after: usize) -> Self {
        self.props.cutoff = cutoff;
        self.props.cutoff_after = cutoff_after;
        self
    }

    pub fn focus(mut self, focus: bool) -> Self {
        self.props.focus = focus;
        self
    }
}

impl<'a, A> View<(), A> for Footer<'a, A> {
    fn new(_state: &(), action_tx: UnboundedSender<A>) -> Self {
        Self {
            action_tx: action_tx.clone(),
            props: FooterProps::default(),
        }
        .move_with_state(&())
    }

    fn move_with_state(self, _state: &()) -> Self {
        Self { ..self }
    }

    fn handle_key_event(&mut self, _key: Key) {}
}

impl<'a, A> Footer<'a, A> {
    fn render_cell(
        &self,
        frame: &mut ratatui::Frame,
        area: Rect,
        block_type: FooterBlockType,
        text: impl Into<Text<'a>>,
        focus: bool,
    ) {
        let footer_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Min(1)])
            .vertical_margin(1)
            .horizontal_margin(1)
            .split(area);

        let footer_block = FooterBlock::default()
            .border_style(style::border(focus))
            .block_type(block_type);
        frame.render_widget(footer_block, area);
        frame.render_widget(text.into(), footer_layout[0]);
    }
}

impl<'a, A, B: Backend> Render<B, ()> for Footer<'a, A> {
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let widths = self
            .props
            .columns
            .iter()
            .map(|c| match c.width {
                Constraint::Min(min) => Constraint::Length(min.saturating_add(3)),
                _ => c.width,
            })
            .collect::<Vec<_>>();

        let layout = Layout::horizontal(widths).split(area);
        let cells = self
            .props
            .columns
            .iter()
            .map(|c| c.text.clone())
            .zip(layout.iter())
            .collect::<Vec<_>>();

        let last = cells.len().saturating_sub(1);
        let len = cells.len();

        for (i, (cell, area)) in cells.into_iter().enumerate() {
            let block_type = match i {
                0 if len == 1 => FooterBlockType::Single,
                0 => FooterBlockType::Begin,
                _ if i == last => FooterBlockType::End,
                _ => FooterBlockType::Repeat,
            };
            self.render_cell(frame, *area, block_type, cell.clone(), self.props.focus);
        }
    }
}
