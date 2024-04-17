use std::any::Any;
use std::fmt::Debug;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::prelude::*;
use ratatui::widgets::{BorderType, Borders, Row};

use crate::ui::ext::{FooterBlock, FooterBlockType, HeaderBlock};
use crate::ui::theme::style;

use super::{Column, EventCallback, UpdateCallback, View, Widget};

#[derive(Clone, Debug)]
pub struct HeaderProps<'a> {
    pub columns: Vec<Column<'a>>,
    pub cutoff: usize,
    pub cutoff_after: usize,
    pub focus: bool,
}

impl<'a> HeaderProps<'a> {
    pub fn columns(mut self, columns: Vec<Column<'a>>) -> Self {
        self.columns = columns;
        self
    }

    pub fn focus(mut self, focus: bool) -> Self {
        self.focus = focus;
        self
    }

    pub fn cutoff(mut self, cutoff: usize, cutoff_after: usize) -> Self {
        self.cutoff = cutoff;
        self.cutoff_after = cutoff_after;
        self
    }
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

pub struct Header<'a, S, A> {
    /// Internal props
    props: HeaderProps<'a>,
    /// Message sender
    action_tx: UnboundedSender<A>,
    /// Custom update handler
    on_update: Option<UpdateCallback<S>>,
    /// Additional custom event handler
    on_change: Option<EventCallback<A>>,
}

impl<'a, S, A> Header<'a, S, A> {
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

impl<'a: 'static, S, A> View<S, A> for Header<'a, S, A> {
    fn new(_state: &S, action_tx: UnboundedSender<A>) -> Self {
        Self {
            action_tx: action_tx.clone(),
            props: HeaderProps::default(),
            on_update: None,
            on_change: None,
        }
    }

    fn on_update(mut self, callback: UpdateCallback<S>) -> Self {
        self.on_update = Some(callback);
        self
    }

    fn on_change(mut self, callback: EventCallback<A>) -> Self {
        self.on_change = Some(callback);
        self
    }

    fn update(&mut self, state: &S) {
        if let Some(on_update) = self.on_update {
            if let Some(props) = (on_update)(state).downcast_ref::<HeaderProps<'_>>() {
                self.props = props.clone();
            }
        }
    }

    fn handle_key_event(&mut self, _key: Key) {
        if let Some(on_change) = self.on_change {
            (on_change)(&self.props, self.action_tx.clone());
        }
    }
}

impl<'a: 'static, S, A, B: Backend> Widget<S, A, B> for Header<'a, S, A> {
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, props: &dyn Any) {
        let props = props
            .downcast_ref::<HeaderProps<'_>>()
            .unwrap_or(&self.props);

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
                    Some(column.text.clone())
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

#[derive(Clone, Debug)]
pub struct FooterProps<'a> {
    pub columns: Vec<Column<'a>>,
    pub cutoff: usize,
    pub cutoff_after: usize,
    pub focus: bool,
}

impl<'a> FooterProps<'a> {
    pub fn columns(mut self, columns: Vec<Column<'a>>) -> Self {
        self.columns = columns;
        self
    }

    pub fn cutoff(mut self, cutoff: usize, cutoff_after: usize) -> Self {
        self.cutoff = cutoff;
        self.cutoff_after = cutoff_after;
        self
    }

    pub fn focus(mut self, focus: bool) -> Self {
        self.focus = focus;
        self
    }
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

pub struct Footer<'a, S, A> {
    /// Internal properties
    props: FooterProps<'a>,
    /// Message sender
    action_tx: UnboundedSender<A>,
    /// Custom update handler
    on_update: Option<UpdateCallback<S>>,
    /// Additional custom event handler
    on_change: Option<EventCallback<A>>,
}

impl<'a, S, A> Footer<'a, S, A> {
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

impl<'a: 'static, S, A> View<S, A> for Footer<'a, S, A> {
    fn new(_state: &S, action_tx: UnboundedSender<A>) -> Self {
        Self {
            action_tx: action_tx.clone(),
            props: FooterProps::default(),
            on_update: None,
            on_change: None,
        }
    }

    fn on_update(mut self, callback: UpdateCallback<S>) -> Self {
        self.on_update = Some(callback);
        self
    }

    fn on_change(mut self, callback: EventCallback<A>) -> Self {
        self.on_change = Some(callback);
        self
    }

    fn update(&mut self, state: &S) {
        if let Some(on_update) = self.on_update {
            if let Some(props) = (on_update)(state).downcast_ref::<FooterProps<'_>>() {
                self.props = props.clone();
            }
        }
    }

    fn handle_key_event(&mut self, _key: Key) {
        if let Some(on_change) = self.on_change {
            (on_change)(&self.props, self.action_tx.clone());
        }
    }
}

impl<'a, S, A> Footer<'a, S, A> {
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

impl<'a: 'static, S, A, B: Backend> Widget<S, A, B> for Footer<'a, S, A> {
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, props: &dyn Any) {
        let props = props
            .downcast_ref::<FooterProps<'_>>()
            .unwrap_or(&self.props);

        let widths = props
            .columns
            .iter()
            .map(|c| match c.width {
                Constraint::Min(min) => Constraint::Length(min.saturating_add(3)),
                _ => c.width,
            })
            .collect::<Vec<_>>();

        let layout = Layout::horizontal(widths).split(area);
        let cells = props
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
            self.render_cell(frame, *area, block_type, cell.clone(), props.focus);
        }
    }
}
