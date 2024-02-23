use std::fmt::Debug;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::prelude::*;
use ratatui::widgets::{BorderType, Borders, Row};

use crate::flux::ui::ext::{FooterBlock, HeaderBlock};
use crate::flux::ui::theme::style;

use super::{Render, Widget};

#[derive(Debug)]
pub struct FooterProps<'a, const W: usize> {
    pub cells: [Text<'a>; W],
    pub widths: [Constraint; W],
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

    fn name(&self) -> &str {
        "footer"
    }

    fn handle_key_event(&mut self, _key: Key) {}
}

impl<'a, A, const W: usize> Render<FooterProps<'a, W>> for Footer<A> {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, props: FooterProps<W>) {
        let widths = props.widths.to_vec();
        let widths = if area.width < props.cutoff as u16 {
            widths.iter().take(props.cutoff_after).collect::<Vec<_>>()
        } else {
            widths.iter().collect::<Vec<_>>()
        };

        let footer_block = FooterBlock::default()
            .borders(Borders::ALL)
            .border_style(style::border(props.focus))
            .border_type(BorderType::Rounded);

        let footer_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Min(1)])
            .vertical_margin(1)
            .horizontal_margin(1)
            .split(area);

        let footer = ratatui::widgets::Table::default()
            .column_spacing(1)
            .header(Row::new(props.cells))
            .widths(widths);

        frame.render_widget(footer_block, area);
        frame.render_widget(footer, footer_layout[0]);
    }
}

#[derive(Debug)]
pub struct HeaderProps<'a, const W: usize> {
    pub cells: [Text<'a>; W],
    pub widths: [Constraint; W],
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

    fn name(&self) -> &str {
        "footer"
    }

    fn handle_key_event(&mut self, _key: Key) {}
}

impl<'a, A, const W: usize> Render<HeaderProps<'a, W>> for Header<A> {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, props: HeaderProps<W>) {
        let widths = props.widths.to_vec();
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

        let header = Row::new(props.cells).style(style::reset().bold());
        let header = ratatui::widgets::Table::default()
            .column_spacing(1)
            .header(header)
            .widths(widths.clone());

        frame.render_widget(block, area);
        frame.render_widget(header, header_layout[0]);
    }
}
