use std::marker::PhantomData;

use termion::event::Key;

use ratatui::layout::{Constraint, Layout};
use ratatui::text::Text;

use super::{RenderProps, View, ViewProps, ViewState};

#[derive(Clone)]
pub struct ParagraphProps<'a> {
    pub content: Text<'a>,
    pub has_header: bool,
    pub has_footer: bool,
    pub page_size: usize,
    pub progress: usize,
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
}

impl<'a> Default for ParagraphProps<'a> {
    fn default() -> Self {
        Self {
            content: Text::raw(""),
            has_header: false,
            has_footer: false,
            page_size: 1,
            progress: 0,
        }
    }
}

#[derive(Clone)]
struct ParagraphState {
    /// Internal offset
    pub offset: usize,
    /// Internal progress
    pub progress: usize,
}

pub struct Paragraph<'a, S, M> {
    /// Internal props
    props: ParagraphProps<'a>,
    /// Internal state
    state: ParagraphState,
    /// Phantom
    phantom: PhantomData<(S, M)>,
}

impl<'a, S, M> Default for Paragraph<'a, S, M> {
    fn default() -> Self {
        Self {
            props: ParagraphProps::default(),
            state: ParagraphState {
                offset: 0,
                progress: 0,
            },
            phantom: PhantomData,
        }
    }
}

impl<'a, S, M> Paragraph<'a, S, M> {
    pub fn scroll(&self) -> (u16, u16) {
        (self.state.offset as u16, 0)
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
        self.state.offset = self.state.offset.saturating_sub(1);
        self.state.progress = Self::scroll_percent(self.state.offset, len, page_size);
        self.scroll()
    }

    fn next(&mut self, len: usize, page_size: usize) -> (u16, u16) {
        if self.state.progress < 100 {
            self.state.offset = self.state.offset.saturating_add(1);
            self.state.progress = Self::scroll_percent(self.state.offset, len, page_size);
        }

        self.scroll()
    }

    fn prev_page(&mut self, len: usize, page_size: usize) -> (u16, u16) {
        self.state.offset = self.state.offset.saturating_sub(page_size);
        self.state.progress = Self::scroll_percent(self.state.offset, len, page_size);
        self.scroll()
    }

    fn next_page(&mut self, len: usize, page_size: usize) -> (u16, u16) {
        let end = len.saturating_sub(page_size);

        self.state.offset = std::cmp::min(self.state.offset.saturating_add(page_size), end);
        self.state.progress = Self::scroll_percent(self.state.offset, len, page_size);
        self.scroll()
    }

    fn begin(&mut self, len: usize, page_size: usize) -> (u16, u16) {
        self.state.offset = 0;
        self.state.progress = Self::scroll_percent(self.state.offset, len, page_size);
        self.scroll()
    }

    fn end(&mut self, len: usize, page_size: usize) -> (u16, u16) {
        self.state.offset = len.saturating_sub(page_size);
        self.state.progress = Self::scroll_percent(self.state.offset, len, page_size);
        self.scroll()
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

impl<'a, S, M> View for Paragraph<'a, S, M>
where
    'a: 'static,
    S: 'static,
    M: 'static,
{
    type Message = M;
    type State = S;

    fn handle_event(&mut self, key: Key) -> Option<Self::Message> {
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

        None
    }

    fn update(&mut self, _state: &Self::State, props: Option<ViewProps>) {
        if let Some(props) = props.and_then(|props| props.inner::<ParagraphProps>()) {
            self.props = props;
        }
    }

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let [content_area] = Layout::horizontal([Constraint::Min(1)])
            .horizontal_margin(1)
            .areas(props.area);
        let content = ratatui::widgets::Paragraph::new(self.props.content.clone())
            .scroll((self.state.offset as u16, 0));

        frame.render_widget(content, content_area);
    }

    fn view_state(&self) -> Option<ViewState> {
        Some(ViewState::USize(self.state.progress))
    }
}
