use std::fmt::Debug;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, Row};

use crate::ui::ext::{FooterBlock, FooterBlockType, HeaderBlock};
use crate::ui::theme::style;

use super::{BaseView, BoxedWidget, Properties, RenderProps, Widget, WidgetState};

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

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
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

        let widths = if props.area.width < self.props.cutoff as u16 {
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
            .border_style(style::border(props.focus))
            .border_type(BorderType::Rounded);

        let header_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Min(1)])
            .vertical_margin(1)
            .horizontal_margin(1)
            .split(props.area);

        let header = Row::new(cells).style(style::reset().bold());
        let header = ratatui::widgets::Table::default()
            .column_spacing(1)
            .header(header)
            .widths(widths.clone());

        frame.render_widget(block, props.area);
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

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let widths = self
            .props
            .columns
            .iter()
            .map(|c| match c.width {
                Constraint::Min(min) => Constraint::Length(min.saturating_add(3)),
                _ => c.width,
            })
            .collect::<Vec<_>>();

        let layout = Layout::horizontal(widths).split(props.area);
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
            self.render_cell(frame, *area, block_type, cell.clone(), props.focus);
        }
    }

    fn base_mut(&mut self) -> &mut BaseView<S, A> {
        &mut self.base
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

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let header_h = if self.header.is_some() { 3 } else { 0 };
        let footer_h = if self.footer.is_some() && !self.props.hide_footer {
            3
        } else {
            0
        };

        let [header_area, content_area, footer_area] = Layout::vertical([
            Constraint::Length(header_h),
            Constraint::Min(1),
            Constraint::Length(footer_h),
        ])
        .areas(props.area);

        let borders = match (
            self.header.is_some(),
            (self.footer.is_some() && !self.props.hide_footer),
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
            header.render(frame, RenderProps::from(header_area).focus(props.focus));
        }

        if let Some(content) = &self.content {
            content.render(
                frame,
                RenderProps::from(block.inner(content_area)).focus(props.focus),
            );
        }

        if let Some(footer) = &self.footer {
            footer.render(frame, RenderProps::from(footer_area).focus(props.focus));
        }
    }

    fn base_mut(&mut self) -> &mut BaseView<S, A> {
        &mut self.base
    }
}

#[derive(Clone)]
pub struct SectionGroupState {
    /// Index of currently focused section.
    focus: Option<usize>,
}

impl WidgetState for SectionGroupState {}

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

impl Properties for SectionGroupProps {}

pub struct SectionGroup<S, A> {
    /// Internal base
    base: BaseView<S, A>,
    /// Internal table properties
    props: SectionGroupProps,
    /// All sections
    sections: Vec<BoxedWidget<S, A>>,
    /// Internal selection and offset state
    state: SectionGroupState,
}

impl<S, A> SectionGroup<S, A> {
    pub fn section(mut self, section: BoxedWidget<S, A>) -> Self {
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

impl<S, A> Widget for SectionGroup<S, A> {
    type State = S;
    type Action = A;

    fn new(_state: &S, action_tx: UnboundedSender<A>) -> Self {
        Self {
            base: BaseView {
                action_tx: action_tx.clone(),
                on_update: None,
                on_event: None,
            },
            props: SectionGroupProps::default(),
            sections: vec![],
            state: SectionGroupState { focus: Some(0) },
        }
    }

    fn handle_event(&mut self, key: Key) {
        if let Some(section) = self
            .state
            .focus
            .and_then(|focus| self.sections.get_mut(focus))
        {
            section.handle_event(key);
        }

        if self.props.handle_keys {
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

        if let Some(on_event) = self.base.on_event {
            (on_event)(
                self.state.clone().to_boxed_any(),
                self.base.action_tx.clone(),
            );
        }
    }

    fn update(&mut self, state: &S) {
        self.props = SectionGroupProps::from_callback(self.base.on_update, state)
            .unwrap_or(self.props.clone());

        for section in &mut self.sections {
            section.update(state);
        }
    }

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let areas = props.layout.split(props.area);

        for (index, area) in areas.iter().enumerate() {
            if let Some(section) = self.sections.get(index) {
                let focus = self
                    .state
                    .focus
                    .map(|focus_index| index == focus_index)
                    .unwrap_or_default();

                section.render(frame, RenderProps::from(*area).focus(focus));
            }
        }
    }

    fn base_mut(&mut self) -> &mut BaseView<S, A> {
        &mut self.base
    }
}
