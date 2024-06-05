use std::marker::PhantomData;

use termion::event::Key;

use ratatui::layout::{Constraint, Layout};
use ratatui::text::Text;
use ratatui::Frame;

use super::utils;
use super::{RenderProps, View, ViewProps, ViewState};

#[derive(Clone)]
pub struct TextAreaProps<'a> {
    pub content: Text<'a>,
    pub has_header: bool,
    pub has_footer: bool,
    pub page_size: usize,
    pub progress: usize,
    pub can_scroll: bool,
}

impl<'a> TextAreaProps<'a> {
    pub fn page_size(mut self, page_size: usize) -> Self {
        self.page_size = page_size;
        self
    }

    pub fn text(mut self, text: &Text<'a>) -> Self {
        self.content = text.clone();
        self
    }

    pub fn can_scroll(mut self, can_scroll: bool) -> Self {
        self.can_scroll = can_scroll;
        self
    }
}

impl<'a> Default for TextAreaProps<'a> {
    fn default() -> Self {
        Self {
            content: Text::raw(""),
            has_header: false,
            has_footer: false,
            page_size: 1,
            progress: 0,
            can_scroll: true,
        }
    }
}

#[derive(Clone)]
struct TextAreaState {
    /// Internal offset
    pub offset: usize,
    /// Internal progress
    pub progress: usize,
}

pub struct TextArea<S, M> {
    /// Internal state
    state: TextAreaState,
    /// Phantom
    phantom: PhantomData<(S, M)>,
}

impl<S, M> Default for TextArea<S, M> {
    fn default() -> Self {
        Self {
            state: TextAreaState {
                offset: 0,
                progress: 0,
            },
            phantom: PhantomData,
        }
    }
}

impl<S, M> TextArea<S, M> {
    fn scroll(&self) -> (u16, u16) {
        (self.state.offset as u16, 0)
    }

    fn prev(&mut self, len: usize, page_size: usize) -> (u16, u16) {
        self.state.offset = self.state.offset.saturating_sub(1);
        self.state.progress = utils::scroll::percent_absolute(self.state.offset, len, page_size);
        self.scroll()
    }

    fn next(&mut self, len: usize, page_size: usize) -> (u16, u16) {
        if self.state.progress < 100 {
            self.state.offset = self.state.offset.saturating_add(1);
            self.state.progress =
                utils::scroll::percent_absolute(self.state.offset, len, page_size);
        }

        self.scroll()
    }

    fn prev_page(&mut self, len: usize, page_size: usize) -> (u16, u16) {
        self.state.offset = self.state.offset.saturating_sub(page_size);
        self.state.progress = utils::scroll::percent_absolute(self.state.offset, len, page_size);
        self.scroll()
    }

    fn next_page(&mut self, len: usize, page_size: usize) -> (u16, u16) {
        let end = len.saturating_sub(page_size);

        self.state.offset = std::cmp::min(self.state.offset.saturating_add(page_size), end);
        self.state.progress = utils::scroll::percent_absolute(self.state.offset, len, page_size);
        self.scroll()
    }

    fn begin(&mut self, len: usize, page_size: usize) -> (u16, u16) {
        self.state.offset = 0;
        self.state.progress = utils::scroll::percent_absolute(self.state.offset, len, page_size);
        self.scroll()
    }

    fn end(&mut self, len: usize, page_size: usize) -> (u16, u16) {
        self.state.offset = len.saturating_sub(page_size);
        self.state.progress = utils::scroll::percent_absolute(self.state.offset, len, page_size);
        self.scroll()
    }
}

impl<S, M> View for TextArea<S, M>
where
    S: 'static,
    M: 'static,
{
    type Message = M;
    type State = S;

    fn handle_event(&mut self, props: Option<&ViewProps>, key: Key) -> Option<Self::Message> {
        let default = TextAreaProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<TextAreaProps>())
            .unwrap_or(&default);

        let len = props.content.lines.len() + 1;
        let page_size = props.page_size;

        if props.can_scroll {
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

        None
    }

    fn render(&self, props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let default = TextAreaProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<TextAreaProps>())
            .unwrap_or(&default);

        let [content_area] = Layout::horizontal([Constraint::Min(1)])
            .horizontal_margin(1)
            .areas(render.area);
        let content = ratatui::widgets::Paragraph::new(props.content.clone())
            .style(props.content.style)
            .scroll((self.state.offset as u16, 0));

        frame.render_widget(content, content_area);
    }

    fn view_state(&self) -> Option<ViewState> {
        Some(ViewState::USize(self.state.progress))
    }
}
