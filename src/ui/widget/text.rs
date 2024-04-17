use std::any::Any;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Text;
use ratatui::widgets::{Block, BorderType, Borders};

use crate::ui::theme::style;

use super::{EventCallback, UpdateCallback, View, Widget};

#[derive(Clone)]
pub struct ParagraphProps<'a> {
    pub content: Text<'a>,
    pub focus: bool,
    pub has_header: bool,
    pub has_footer: bool,
    pub page_size: usize,
}

impl<'a> ParagraphProps<'a> {
    pub fn page_size(mut self, page_size: usize) -> Self {
        self.page_size = page_size;
        self
    }

    pub fn text(mut self, text: &Text<'a>) -> Self {
        self.content = text.clone();
        self
    }

    pub fn focus(mut self, focus: bool) -> Self {
        self.focus = focus;
        self
    }

    pub fn to_boxed(self) -> Box<Self> {
        Box::new(self)
    }
}

impl<'a> Default for ParagraphProps<'a> {
    fn default() -> Self {
        Self {
            content: Text::raw(""),
            focus: false,
            has_header: false,
            has_footer: false,
            page_size: 1,
        }
    }
}

pub struct Paragraph<'a, S, A> {
    /// Internal properties
    props: ParagraphProps<'a>,
    /// Message sender
    action_tx: UnboundedSender<A>,
    /// Custom update handler
    on_update: Option<UpdateCallback<S>>,
    /// Additional custom event handler
    on_change: Option<EventCallback<A>>,
    /// Internal offset
    offset: usize,
    /// Internal progress
    progress: usize,
}

impl<'a, S, A> Paragraph<'a, S, A> {
    pub fn scroll(&self) -> (u16, u16) {
        (self.offset as u16, 0)
    }

    pub fn page_size(mut self, page_size: usize) -> Self {
        self.props.page_size = page_size;
        self
    }

    pub fn text(mut self, text: &Text<'a>) -> Self {
        self.props.content = text.clone();
        self
    }

    fn prev(&mut self, len: usize, page_size: usize) -> (u16, u16) {
        self.offset = self.offset.saturating_sub(1);
        self.progress = Self::scroll_percent(self.offset, len, page_size);
        self.scroll()
    }

    fn next(&mut self, len: usize, page_size: usize) -> (u16, u16) {
        if self.progress < 100 {
            self.offset = self.offset.saturating_add(1);
            self.progress = Self::scroll_percent(self.offset, len, page_size);
        }

        self.scroll()
    }

    fn prev_page(&mut self, len: usize, page_size: usize) -> (u16, u16) {
        self.offset = self.offset.saturating_sub(page_size);
        self.progress = Self::scroll_percent(self.offset, len, page_size);
        self.scroll()
    }

    fn next_page(&mut self, len: usize, page_size: usize) -> (u16, u16) {
        let end = len.saturating_sub(page_size);

        self.offset = std::cmp::min(self.offset.saturating_add(page_size), end);
        self.progress = Self::scroll_percent(self.offset, len, page_size);
        self.scroll()
    }

    fn begin(&mut self, len: usize, page_size: usize) -> (u16, u16) {
        self.offset = 0;
        self.progress = Self::scroll_percent(self.offset, len, page_size);
        self.scroll()
    }

    fn end(&mut self, len: usize, page_size: usize) -> (u16, u16) {
        self.offset = len.saturating_sub(page_size);
        self.progress = Self::scroll_percent(self.offset, len, page_size);
        self.scroll()
    }

    pub fn progress(&self) -> usize {
        self.progress
    }

    fn scroll_percent(offset: usize, len: usize, height: usize) -> usize {
        if height >= len {
            100
        } else {
            let y = offset as f64;
            let h = height as f64;
            let t = len.saturating_sub(1) as f64;
            let v = y / (t - h) * 100_f64;

            std::cmp::max(0, std::cmp::min(100, v as usize))
        }
    }
}

impl<'a: 'static, S, A> View<S, A> for Paragraph<'a, S, A> {
    fn new(_state: &S, action_tx: UnboundedSender<A>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
            offset: 0,
            progress: 0,
            props: ParagraphProps::default(),
            on_update: None,
            on_change: None,
        }
    }

    fn on_change(mut self, callback: EventCallback<A>) -> Self {
        self.on_change = Some(callback);
        self
    }

    fn on_update(mut self, callback: UpdateCallback<S>) -> Self {
        self.on_update = Some(callback);
        self
    }

    fn update(&mut self, state: &S) {
        if let Some(on_update) = self.on_update {
            if let Some(props) = (on_update)(state).downcast_ref::<ParagraphProps>() {
                self.props = props.clone();
            }
        }
    }

    fn handle_key_event(&mut self, key: Key) {
        let len = self.props.content.lines.len() + 1;
        let page_size = self.props.page_size;

        match key {
            Key::Up | Key::Char('k') => {
                self.prev(len, page_size);
            }
            Key::Down | Key::Char('j') => {
                self.next(len, page_size);
            }
            Key::PageUp => {
                self.prev_page(len, page_size);
            }
            Key::PageDown => {
                self.next_page(len, page_size);
            }
            Key::Home => {
                self.begin(len, page_size);
            }
            Key::End => {
                self.end(len, page_size);
            }
            _ => {}
        }

        if let Some(on_change) = self.on_change {
            (on_change)(&self.props, self.action_tx.clone());
        }
    }
}

impl<'a: 'static, B, S, A> Widget<B, S, A> for Paragraph<'a, S, A>
where
    B: Backend,
{
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, props: &dyn Any) {
        let props = props
            .downcast_ref::<ParagraphProps>()
            .unwrap_or(&self.props);

        let block = Block::default()
            .borders(Borders::LEFT | Borders::RIGHT)
            .border_type(BorderType::Rounded)
            .border_style(style::border(props.focus));
        frame.render_widget(block, area);

        let [content_area] = Layout::horizontal([Constraint::Min(1)])
            .horizontal_margin(2)
            .areas(area);
        let content =
            ratatui::widgets::Paragraph::new(props.content.clone()).scroll((self.offset as u16, 0));

        frame.render_widget(content, content_area);
    }
}
