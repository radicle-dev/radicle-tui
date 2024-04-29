use std::any::Any;
use std::fmt::Debug;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, Row};

use crate::ui::ext::{FooterBlock, FooterBlockType, HeaderBlock};
use crate::ui::theme::style;

use super::{BaseView, BoxedWidget, Column, Properties, Widget};

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

impl<'a: 'static> Properties for HeaderProps<'a> {}

pub struct Header<'a: 'static, S, A> {
    /// Internal props
    props: HeaderProps<'a>,
    /// Internal base
    base: BaseView<S, A>,
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

impl<'a: 'static, S, A> Widget for Header<'a, S, A> {
    type Action = A;
    type State = S;

    fn new(_state: &S, action_tx: UnboundedSender<A>) -> Self {
        Self {
            base: BaseView {
                action_tx: action_tx.clone(),
                on_update: None,
                on_event: None,
            },
            props: HeaderProps::default(),
        }
    }

    fn handle_event(&mut self, _key: Key) {}

    fn update(&mut self, state: &S) {
        self.props = self
            .base
            .on_update
            .and_then(|on_update| HeaderProps::from_boxed_any((on_update)(state)))
            .unwrap_or(self.props.clone());
    }

    fn render(&self, frame: &mut ratatui::Frame, area: Rect, props: Option<&dyn Any>) {
        let props = props
            .and_then(|props| props.downcast_ref::<HeaderProps>())
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

    fn base_mut(&mut self) -> &mut BaseView<S, A> {
        &mut self.base
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

impl<'a: 'static> Properties for FooterProps<'a> {}

pub struct Footer<'a, S, A> {
    /// Internal props
    props: FooterProps<'a>,
    /// Internal base
    base: BaseView<S, A>,
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

impl<'a: 'static, S, A> Widget for Footer<'a, S, A> {
    type Action = A;
    type State = S;

    fn new(_state: &S, action_tx: UnboundedSender<A>) -> Self {
        Self {
            base: BaseView {
                action_tx: action_tx.clone(),
                on_update: None,
                on_event: None,
            },
            props: FooterProps::default(),
        }
    }

    fn handle_event(&mut self, _key: Key) {}

    fn update(&mut self, state: &S) {
        self.props = self
            .base
            .on_update
            .and_then(|on_update| FooterProps::from_boxed_any((on_update)(state)))
            .unwrap_or(self.props.clone());
    }

    fn render(&self, frame: &mut ratatui::Frame, area: Rect, props: Option<&dyn Any>) {
        let props = props
            .and_then(|props| props.downcast_ref::<FooterProps>())
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

    fn base_mut(&mut self) -> &mut BaseView<S, A> {
        &mut self.base
    }
}

#[derive(Clone, Default)]
pub struct ContainerProps {
    focus: bool,
    hide_footer: bool,
}

impl ContainerProps {
    pub fn hide_footer(mut self, hide: bool) -> Self {
        self.hide_footer = hide;
        self
    }

    pub fn focus(mut self, focus: bool) -> Self {
        self.focus = focus;
        self
    }
}

impl Properties for ContainerProps {}

pub struct Container<S, A> {
    /// Internal base
    base: BaseView<S, A>,
    /// Internal props
    props: ContainerProps,
    /// Container header
    header: Option<BoxedWidget<S, A>>,
    /// Content widget
    content: Option<BoxedWidget<S, A>>,
    /// Container footer
    footer: Option<BoxedWidget<S, A>>,
}

impl<S, A> Container<S, A> {
    pub fn header(mut self, header: BoxedWidget<S, A>) -> Self {
        self.header = Some(header);
        self
    }

    pub fn content(mut self, content: BoxedWidget<S, A>) -> Self {
        self.content = Some(content);
        self
    }

    pub fn footer(mut self, footer: BoxedWidget<S, A>) -> Self {
        self.footer = Some(footer);
        self
    }
}

impl<S, A> Widget for Container<S, A> {
    type Action = A;
    type State = S;

    fn new(_state: &S, action_tx: UnboundedSender<A>) -> Self
    where
        Self: Sized,
    {
        Self {
            base: BaseView {
                action_tx: action_tx.clone(),

                on_update: None,
                on_event: None,
            },
            props: ContainerProps::default(),
            header: None,
            content: None,
            footer: None,
        }
    }

    fn handle_event(&mut self, key: termion::event::Key) {
        if let Some(content) = &mut self.content {
            content.handle_event(key);
        }
    }

    fn update(&mut self, state: &S) {
        self.props =
            ContainerProps::from_callback(self.base.on_update, state).unwrap_or(self.props.clone());

        if let Some(header) = &mut self.header {
            header.update(state);
        }

        if let Some(content) = &mut self.content {
            content.update(state);
        }

        if let Some(footer) = &mut self.footer {
            footer.update(state);
        }
    }

    fn render(&self, frame: &mut ratatui::Frame, area: Rect, props: Option<&dyn Any>) {
        let props = props
            .and_then(|props| props.downcast_ref::<ContainerProps>())
            .unwrap_or(&self.props);

        let header_h = if self.header.is_some() { 3 } else { 0 };
        let footer_h = if self.footer.is_some() && !props.hide_footer {
            3
        } else {
            0
        };

        let [header_area, content_area, footer_area] = Layout::vertical([
            Constraint::Length(header_h),
            Constraint::Min(1),
            Constraint::Length(footer_h),
        ])
        .areas(area);

        let borders = match (
            self.header.is_some(),
            (self.footer.is_some() && !props.hide_footer),
        ) {
            (false, false) => Borders::ALL,
            (true, false) => Borders::BOTTOM | Borders::LEFT | Borders::RIGHT,
            (false, true) => Borders::TOP | Borders::LEFT | Borders::RIGHT,
            (true, true) => Borders::LEFT | Borders::RIGHT,
        };

        let block = Block::default()
            .border_style(style::border(props.focus))
            .border_type(BorderType::Rounded)
            .borders(borders);
        frame.render_widget(block.clone(), content_area);

        if let Some(header) = &self.header {
            header.render(frame, header_area, None);
        }

        if let Some(content) = &self.content {
            content.render(frame, block.inner(content_area), None);
        }

        if let Some(footer) = &self.footer {
            footer.render(frame, footer_area, None);
        }
    }

    fn base_mut(&mut self) -> &mut BaseView<S, A> {
        &mut self.base
    }
}
