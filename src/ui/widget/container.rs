use std::fmt::Debug;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::prelude::*;
use ratatui::widgets::{BorderType, Borders, Row};

use crate::ui::ext::{FooterBlock, FooterBlockType, HeaderBlock};
use crate::ui::theme::style;

use super::{Column, Render, Widget};

#[derive(Debug)]
pub struct FooterProps<'a> {
    pub cells: Vec<Text<'a>>,
    pub widths: Vec<Constraint>,
    pub cutoff: usize,
    pub cutoff_after: usize,
    pub focus: bool,
}

pub struct Footer<A> {
    /// Sending actions to the state store
    pub action_tx: UnboundedSender<A>,
}

impl<S, A> Widget<S, A> for Footer<A> {
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

    fn handle_key_event(&mut self, _key: Key) {}
}

impl<A> Footer<A> {
    fn render_cell<'a>(
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

impl<'a, A> Render<FooterProps<'a>> for Footer<A> {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, props: FooterProps) {
        let widths = props
            .widths
            .into_iter()
            .map(|c| match c {
                Constraint::Min(min) => Constraint::Length(min.saturating_add(3)),
                _ => c,
            })
            .collect::<Vec<_>>();

        let layout = Layout::horizontal(widths).split(area);
        let cells = props.cells.iter().zip(layout.iter()).collect::<Vec<_>>();

        let last = cells.len().saturating_sub(1);
        let len = cells.len();

        for (i, (cell, area)) in cells.into_iter().enumerate() {
            let block_type = match i {
                0 if len == 1 => FooterBlockType::Single,
                0 => FooterBlockType::Begin,
                _ if i == last => FooterBlockType::End,
                _ => FooterBlockType::Repeat,
            };
            self.render_cell(frame, *area, block_type, cell.clone(), props.focus);
        }
    }
}

#[derive(Debug)]
pub struct HeaderProps {
    pub columns: Vec<Column>,
    pub cutoff: usize,
    pub cutoff_after: usize,
    pub focus: bool,
}

pub struct Header<A> {
    /// Sending actions to the state store
    pub action_tx: UnboundedSender<A>,
}

impl<S, A> Widget<S, A> for Header<A> {
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

    fn handle_key_event(&mut self, _key: Key) {}
}

impl<A> Render<HeaderProps> for Header<A> {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, props: HeaderProps) {
        let widths: Vec<Constraint> = props
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
        let cells = props
            .columns
            .iter()
            .filter_map(|column| {
                if !column.skip {
                    Some(column.title.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let widths = if area.width < props.cutoff as u16 {
            widths.iter().take(props.cutoff_after).collect::<Vec<_>>()
        } else {
            widths.iter().collect::<Vec<_>>()
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
