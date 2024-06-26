use std::fmt::Debug;
use std::marker::PhantomData;

use termion::event::Key;

use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, Row};

use crate::ui::ext::{FooterBlock, FooterBlockType, HeaderBlock};
use crate::ui::theme::style;

use super::{RenderProps, View, ViewProps, Widget};

#[derive(Clone, Debug)]
pub struct Column<'a> {
    pub text: Text<'a>,
    pub width: Constraint,
    pub skip: bool,
}

impl<'a> Column<'a> {
    pub fn new(text: impl Into<Text<'a>>, width: Constraint) -> Self {
        Self {
            text: text.into(),
            width,
            skip: false,
        }
    }

    pub fn skip(mut self, skip: bool) -> Self {
        self.skip = skip;
        self
    }
}

#[derive(Clone, Debug)]
pub struct HeaderProps<'a> {
    pub columns: Vec<Column<'a>>,
    pub cutoff: usize,
    pub cutoff_after: usize,
}

impl<'a> HeaderProps<'a> {
    pub fn columns(mut self, columns: Vec<Column<'a>>) -> Self {
        self.columns = columns;
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
        }
    }
}

pub struct Header<S, M> {
    /// Phantom
    phantom: PhantomData<(S, M)>,
}

impl<S, M> Default for Header<S, M> {
    fn default() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<'a: 'static, S, M> View for Header<S, M> {
    type Message = M;
    type State = S;

    fn render(&self, props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let default = HeaderProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<HeaderProps>())
            .unwrap_or(&default);

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

        let widths = if render.area.width < props.cutoff as u16 {
            widths.iter().take(props.cutoff_after).collect::<Vec<_>>()
        } else {
            widths.iter().collect::<Vec<_>>()
        };

        // Render header
        let block = HeaderBlock::default()
            .borders(Borders::ALL)
            .border_style(style::border(render.focus))
            .border_type(BorderType::Rounded);

        let header_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Min(1)])
            .vertical_margin(1)
            .horizontal_margin(1)
            .split(render.area);

        let header = Row::new(cells).style(style::reset().bold());
        let header = ratatui::widgets::Table::default()
            .column_spacing(1)
            .header(header)
            .widths(widths.clone());

        frame.render_widget(block, render.area);
        frame.render_widget(header, header_layout[0]);
    }
}

#[derive(Clone, Debug)]
pub struct FooterProps<'a> {
    pub columns: Vec<Column<'a>>,
    pub cutoff: usize,
    pub cutoff_after: usize,
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
}

impl<'a> Default for FooterProps<'a> {
    fn default() -> Self {
        Self {
            columns: vec![],
            cutoff: usize::MAX,
            cutoff_after: usize::MAX,
        }
    }
}

pub struct Footer<S, M> {
    /// Phantom
    phantom: PhantomData<(S, M)>,
}

impl<S, M> Default for Footer<S, M> {
    fn default() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<'a, S, M> Footer<S, M> {
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

impl<'a: 'static, S, M> View for Footer<S, M> {
    type Message = M;
    type State = S;

    fn render(&self, props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let default = FooterProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<FooterProps>())
            .unwrap_or(&default);

        let widths = props
            .columns
            .iter()
            .map(|c| match c.width {
                Constraint::Min(min) => Constraint::Length(min.saturating_add(3)),
                _ => c.width,
            })
            .collect::<Vec<_>>();

        let layout = Layout::horizontal(widths).split(render.area);
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
            self.render_cell(frame, *area, block_type, cell.clone(), render.focus);
        }
    }
}

#[derive(Clone, Default)]
pub struct ContainerProps {
    hide_footer: bool,
}

impl ContainerProps {
    pub fn hide_footer(mut self, hide: bool) -> Self {
        self.hide_footer = hide;
        self
    }
}

pub struct Container<S, M> {
    /// Container header
    header: Option<Widget<S, M>>,
    /// Content widget
    content: Option<Widget<S, M>>,
    /// Container footer
    footer: Option<Widget<S, M>>,
}

impl<S, M> Default for Container<S, M> {
    fn default() -> Self {
        Self {
            header: None,
            content: None,
            footer: None,
        }
    }
}

impl<S, M> Container<S, M> {
    pub fn header(mut self, header: Widget<S, M>) -> Self {
        self.header = Some(header);
        self
    }

    pub fn content(mut self, content: Widget<S, M>) -> Self {
        self.content = Some(content);
        self
    }

    pub fn footer(mut self, footer: Widget<S, M>) -> Self {
        self.footer = Some(footer);
        self
    }
}

impl<S, M> View for Container<S, M>
where
    S: 'static,
    M: 'static,
{
    type Message = M;
    type State = S;

    fn handle_event(&mut self, _props: Option<&ViewProps>, key: Key) -> Option<Self::Message> {
        if let Some(content) = &mut self.content {
            content.handle_event(key);
        }

        None
    }

    fn update(&mut self, _props: Option<&ViewProps>, state: &Self::State) {
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

    fn render(&self, props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let default = ContainerProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<ContainerProps>())
            .unwrap_or(&default);

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
        .areas(render.area);

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
            .border_style(style::border(render.focus))
            .border_type(BorderType::Rounded)
            .borders(borders);
        frame.render_widget(block.clone(), content_area);

        if let Some(header) = &self.header {
            header.render(RenderProps::from(header_area).focus(render.focus), frame);
        }

        if let Some(content) = &self.content {
            content.render(
                RenderProps::from(block.inner(content_area)).focus(render.focus),
                frame,
            );
        }

        if let Some(footer) = &self.footer {
            footer.render(RenderProps::from(footer_area).focus(render.focus), frame);
        }
    }
}

#[derive(Clone)]
pub struct SectionGroupState {
    /// Index of currently focused section.
    focus: Option<usize>,
}

#[derive(Clone, Default)]
pub struct SectionGroupProps {
    /// If this pages' keys should be handled.
    handle_keys: bool,
}

impl SectionGroupProps {
    pub fn handle_keys(mut self, handle_keys: bool) -> Self {
        self.handle_keys = handle_keys;
        self
    }
}

pub struct SectionGroup<S, M> {
    /// All sections
    sections: Vec<Widget<S, M>>,
    /// Internal selection and offset state
    state: SectionGroupState,
}

impl<S, M> Default for SectionGroup<S, M> {
    fn default() -> Self {
        Self {
            sections: vec![],
            state: SectionGroupState { focus: Some(0) },
        }
    }
}

impl<S, M> SectionGroup<S, M> {
    pub fn section(mut self, section: Widget<S, M>) -> Self {
        self.sections.push(section);
        self
    }

    fn prev(&mut self) -> Option<usize> {
        let focus = self.state.focus.map(|current| current.saturating_sub(1));
        self.state.focus = focus;
        focus
    }

    fn next(&mut self, len: usize) -> Option<usize> {
        let focus = self.state.focus.map(|current| {
            if current < len.saturating_sub(1) {
                current.saturating_add(1)
            } else {
                current
            }
        });
        self.state.focus = focus;
        focus
    }
}

impl<S, M> View for SectionGroup<S, M>
where
    S: 'static,
    M: 'static,
{
    type State = S;
    type Message = M;

    fn handle_event(&mut self, props: Option<&ViewProps>, key: Key) -> Option<Self::Message> {
        let default = SectionGroupProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<SectionGroupProps>())
            .unwrap_or(&default);

        if let Some(section) = self
            .state
            .focus
            .and_then(|focus| self.sections.get_mut(focus))
        {
            section.handle_event(key);
        }

        if props.handle_keys {
            match key {
                Key::Left => {
                    self.prev();
                }
                Key::Right => {
                    self.next(self.sections.len());
                }
                _ => {}
            }
        }

        None
    }

    fn update(&mut self, _props: Option<&ViewProps>, state: &Self::State) {
        for section in &mut self.sections {
            section.update(state);
        }
    }

    fn render(&self, _props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let areas = render.layout.split(render.area);

        for (index, area) in areas.iter().enumerate() {
            if let Some(section) = self.sections.get(index) {
                let focus = self
                    .state
                    .focus
                    .map(|focus_index| index == focus_index)
                    .unwrap_or_default();

                section.render(RenderProps::from(*area).focus(focus), frame);
            }
        }
    }
}
