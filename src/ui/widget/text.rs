use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Text;
use ratatui::widgets::{Block, BorderType, Borders};

use crate::ui::theme::style;

use super::{Render, View};

pub struct ParagraphProps<'a> {
    pub content: Text<'a>,
    pub focus: bool,
    pub has_header: bool,
    pub has_footer: bool,
    pub page_size: usize,
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

pub struct Paragraph<'a, A> {
    /// Sending actions to the state store
    pub action_tx: UnboundedSender<A>,
    /// Internal offset
    offset: usize,
    /// Internal progress
    progress: usize,
    /// Internal properties
    props: ParagraphProps<'a>,
}

impl<'a, A> Paragraph<'a, A> {
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

impl<'a, S, A> View<S, A> for Paragraph<'a, A> {
    fn new(state: &S, action_tx: UnboundedSender<A>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
            offset: 0,
            progress: 0,
            props: ParagraphProps::default(),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, _state: &S) -> Self
    where
        Self: Sized,
    {
        Self { ..self }
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
    }
}

impl<'a, A, B: Backend> Render<B, ()> for Paragraph<'a, A> {
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let block = Block::default()
            .borders(Borders::LEFT | Borders::RIGHT)
            .border_type(BorderType::Rounded)
            .border_style(style::border(self.props.focus));
        frame.render_widget(block, area);

        let [content_area] = Layout::horizontal([Constraint::Min(1)])
            .horizontal_margin(2)
            .areas(area);
        let content = ratatui::widgets::Paragraph::new(self.props.content.clone())
            .scroll((self.offset as u16, 0));

        frame.render_widget(content, content_area);
    }
}
