use std::fmt::Debug;
use std::marker::PhantomData;

use termion::event::Key;

use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, Row};

use crate::ui::ext::{FooterBlock, FooterBlockType, HeaderBlock};
use crate::ui::theme::{style, Theme};
use crate::ui::Column;

use super::{PredefinedLayout, RenderProps, View, ViewProps, ViewState, Widget};

#[derive(Clone, Debug)]
pub struct HeaderProps<'a> {
    pub columns: Vec<Column<'a>>,
    pub cutoff: usize,
    pub cutoff_after: usize,
    pub border_style: Style,
    pub focus_border_style: Style,
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

    pub fn border_style(mut self, color: Style) -> Self {
        self.border_style = color;
        self
    }

    pub fn focus_border_style(mut self, color: Style) -> Self {
        self.focus_border_style = color;
        self
    }
}

impl<'a> Default for HeaderProps<'a> {
    fn default() -> Self {
        let theme = Theme::default();

        Self {
            columns: vec![],
            cutoff: usize::MAX,
            cutoff_after: usize::MAX,
            border_style: theme.border_style,
            focus_border_style: theme.focus_border_style,
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

    fn render(&mut self, props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let default = HeaderProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<HeaderProps>())
            .unwrap_or(&default);

        let width = render.area.width.saturating_sub(2);

        let widths: Vec<Constraint> = props
            .columns
            .iter()
            .filter_map(|c| {
                if !c.skip && c.displayed(width as usize) {
                    Some(c.width)
                } else {
                    None
                }
            })
            .collect();

        let cells = props
            .columns
            .iter()
            .filter_map(|column| {
                if !column.skip && column.displayed(width as usize) {
                    Some(column.text.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let border_style = if render.focus {
            props.focus_border_style
        } else {
            props.border_style
        };

        // Render header
        let block = HeaderBlock::default()
            .borders(Borders::ALL)
            .border_style(border_style)
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
    pub border_style: Style,
    pub focus_border_style: Style,
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

    pub fn border_style(mut self, color: Style) -> Self {
        self.border_style = color;
        self
    }

    pub fn focus_border_style(mut self, color: Style) -> Self {
        self.focus_border_style = color;
        self
    }
}

impl<'a> Default for FooterProps<'a> {
    fn default() -> Self {
        let theme = Theme::default();

        Self {
            columns: vec![],
            cutoff: usize::MAX,
            cutoff_after: usize::MAX,
            border_style: theme.border_style,
            focus_border_style: theme.focus_border_style,
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
        border_style: Style,
        render: RenderProps,
        block_type: FooterBlockType,
        text: impl Into<Text<'a>>,
    ) {
        let footer_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Min(1)])
            .vertical_margin(1)
            .horizontal_margin(1)
            .split(render.area);

        let footer_block = FooterBlock::default()
            .border_style(border_style)
            .block_type(block_type);
        frame.render_widget(footer_block, render.area);
        frame.render_widget(text.into(), footer_layout[0]);
    }
}

impl<'a: 'static, S, M> View for Footer<S, M> {
    type Message = M;
    type State = S;

    fn render(&mut self, props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let default = FooterProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<FooterProps>())
            .unwrap_or(&default);

        let border_style = if render.focus {
            props.focus_border_style
        } else {
            props.border_style
        };

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
                0 if len == 1 => FooterBlockType::Single { top: true },
                0 => FooterBlockType::Begin,
                _ if i == last => FooterBlockType::End,
                _ => FooterBlockType::Repeat,
            };
            self.render_cell(
                frame,
                border_style,
                render.clone().area(*area),
                block_type,
                cell.clone(),
            );
        }
    }
}

#[derive(Clone)]
pub struct ContainerProps {
    hide_footer: bool,
    border_style: Style,
    focus_border_style: Style,
}

impl Default for ContainerProps {
    fn default() -> Self {
        let theme = Theme::default();

        Self {
            hide_footer: false,
            border_style: theme.border_style,
            focus_border_style: theme.focus_border_style,
        }
    }
}

impl ContainerProps {
    pub fn hide_footer(mut self, hide: bool) -> Self {
        self.hide_footer = hide;
        self
    }

    pub fn border_style(mut self, color: Style) -> Self {
        self.border_style = color;
        self
    }

    pub fn focus_border_style(mut self, color: Style) -> Self {
        self.focus_border_style = color;
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

    fn render(&mut self, props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let default = ContainerProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<ContainerProps>())
            .unwrap_or(&default);

        let border_style = if render.focus {
            props.focus_border_style
        } else {
            props.border_style
        };

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
            .border_style(border_style)
            .border_type(BorderType::Rounded)
            .borders(borders);
        frame.render_widget(block.clone(), content_area);

        if let Some(header) = self.header.as_mut() {
            header.render(RenderProps::from(header_area).focus(render.focus), frame);
        }

        if let Some(content) = self.content.as_mut() {
            content.render(
                RenderProps::from(block.inner(content_area)).focus(render.focus),
                frame,
            );
        }

        if let Some(footer) = self.footer.as_mut() {
            footer.render(RenderProps::from(footer_area).focus(render.focus), frame);
        }
    }
}

#[derive(Clone, Default)]
pub enum SplitContainerFocus {
    #[default]
    Top,
    Bottom,
}

#[derive(Clone)]
pub struct SplitContainerProps {
    split_focus: SplitContainerFocus,
    heights: [Constraint; 2],
    border_style: Style,
    focus_border_style: Style,
}

impl Default for SplitContainerProps {
    fn default() -> Self {
        let theme = Theme::default();

        Self {
            split_focus: SplitContainerFocus::default(),
            heights: [Constraint::Percentage(50), Constraint::Percentage(50)],
            border_style: theme.border_style,
            focus_border_style: theme.focus_border_style,
        }
    }
}

impl SplitContainerProps {
    pub fn split_focus(mut self, split_focus: SplitContainerFocus) -> Self {
        self.split_focus = split_focus;
        self
    }

    pub fn heights(mut self, heights: [Constraint; 2]) -> Self {
        self.heights = heights;
        self
    }

    pub fn border_style(mut self, color: Style) -> Self {
        self.border_style = color;
        self
    }

    pub fn focus_border_style(mut self, color: Style) -> Self {
        self.focus_border_style = color;
        self
    }
}

pub struct SplitContainer<S, M> {
    /// Container top
    top: Option<Widget<S, M>>,
    /// Content bottom
    bottom: Option<Widget<S, M>>,
}

impl<S, M> Default for SplitContainer<S, M> {
    fn default() -> Self {
        Self {
            top: None,
            bottom: None,
        }
    }
}

impl<S, M> SplitContainer<S, M> {
    pub fn top(mut self, top: Widget<S, M>) -> Self {
        self.top = Some(top);
        self
    }

    pub fn bottom(mut self, bottom: Widget<S, M>) -> Self {
        self.bottom = Some(bottom);
        self
    }
}

impl<S, M> View for SplitContainer<S, M>
where
    S: 'static,
    M: 'static,
{
    type Message = M;
    type State = S;

    fn handle_event(&mut self, props: Option<&ViewProps>, key: Key) -> Option<Self::Message> {
        let default = SplitContainerProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<SplitContainerProps>())
            .unwrap_or(&default);

        match props.split_focus {
            SplitContainerFocus::Top => {
                if let Some(top) = self.top.as_mut() {
                    top.handle_event(key);
                }
            }
            SplitContainerFocus::Bottom => {
                if let Some(bottom) = self.bottom.as_mut() {
                    bottom.handle_event(key);
                }
            }
        }

        None
    }

    fn update(&mut self, _props: Option<&ViewProps>, state: &Self::State) {
        if let Some(top) = self.top.as_mut() {
            top.update(state);
        }

        if let Some(bottom) = self.bottom.as_mut() {
            bottom.update(state);
        }
    }

    fn render(&mut self, props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let default = SplitContainerProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<SplitContainerProps>())
            .unwrap_or(&default);

        let heights = props
            .heights
            .iter()
            .map(|c| {
                if let Constraint::Length(l) = c {
                    Constraint::Length(l + 2)
                } else {
                    *c
                }
            })
            .collect::<Vec<_>>();

        let border_style = if render.focus {
            props.focus_border_style
        } else {
            props.border_style
        };

        let [top_area, bottom_area] = Layout::vertical(heights).areas(render.area);

        if let Some(top) = self.top.as_mut() {
            let block = HeaderBlock::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .border_type(BorderType::Rounded);

            frame.render_widget(block, top_area);

            let [top_area] = Layout::default()
                .direction(Direction::Vertical)
                .constraints(vec![Constraint::Min(1)])
                .vertical_margin(1)
                .horizontal_margin(1)
                .areas(top_area);
            top.render(RenderProps::from(top_area).focus(render.focus), frame)
        }

        if let Some(bottom) = self.bottom.as_mut() {
            let block = Block::default()
                .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                .border_style(border_style)
                .border_type(BorderType::Rounded);

            frame.render_widget(block, bottom_area);

            let [bottom_area, _] = Layout::default()
                .direction(Direction::Vertical)
                .constraints(vec![Constraint::Min(1), Constraint::Length(1)])
                .horizontal_margin(1)
                .areas(bottom_area);
            bottom.render(RenderProps::from(bottom_area).focus(render.focus), frame)
        }
    }
}

#[derive(Clone, Debug)]
pub struct SectionGroupState {
    /// Index of currently focused section.
    pub focus: Option<usize>,
}

#[derive(Clone, Default)]
pub struct SectionGroupProps {
    /// Index of currently focused section. If set, it will override the widgets'
    /// internal state.
    focus: Option<usize>,
    /// If this pages' keys should be handled.
    handle_keys: bool,
    /// Section layout
    layout: PredefinedLayout,
}

impl SectionGroupProps {
    pub fn handle_keys(mut self, handle_keys: bool) -> Self {
        self.handle_keys = handle_keys;
        self
    }

    pub fn layout(mut self, layout: PredefinedLayout) -> Self {
        self.layout = layout;
        self
    }

    pub fn focus(mut self, focus: Option<usize>) -> Self {
        self.focus = focus;
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
                Key::BackTab => {
                    self.prev();
                }
                Key::Char('\t') => {
                    self.next(self.sections.len());
                }
                _ => {}
            }
        }

        None
    }

    fn update(&mut self, props: Option<&ViewProps>, state: &Self::State) {
        let default = SectionGroupProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<SectionGroupProps>())
            .unwrap_or(&default);

        for section in &mut self.sections {
            section.update(state);
        }

        if props.focus.is_some() && props.focus != self.state.focus {
            self.state.focus = props.focus;
        }
    }

    fn render(&mut self, props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let default = SectionGroupProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<SectionGroupProps>())
            .unwrap_or(&default);

        let areas = props.layout.split(render.area);

        for (index, area) in areas.iter().enumerate() {
            if let Some(section) = self.sections.get_mut(index) {
                let focus = self
                    .state
                    .focus
                    .map(|focus_index| index == focus_index)
                    .unwrap_or_default();

                section.render(RenderProps::from(*area).focus(focus), frame);
            }
        }
    }

    fn view_state(&self) -> Option<super::ViewState> {
        Some(ViewState::SectionGroup(self.state.clone()))
    }
}
