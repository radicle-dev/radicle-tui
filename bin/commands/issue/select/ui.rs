use std::collections::HashMap;
use std::str::FromStr;
use std::vec;

use radicle::issue::{self, CloseReason};
use ratatui::Frame;
use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::layout::{Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};

use radicle_tui as tui;

use tui::ui::items::{IssueItem, IssueItemFilter};
use tui::ui::span;
use tui::ui::widget::container::{
    Column, Container, ContainerProps, Footer, FooterProps, Header, HeaderProps, SectionGroup,
    SectionGroupProps,
};
use tui::ui::widget::input::{TextField, TextFieldProps};
use tui::ui::widget::list::{Table, TableProps, TableUtils};
use tui::ui::widget::text::{Paragraph, ParagraphProps};
use tui::ui::widget::window::{Shortcuts, ShortcutsProps};
use tui::ui::widget::{self, ViewProps};
use tui::ui::widget::{RenderProps, ToWidget, View};

use tui::{BoxedAny, Selection};

use crate::tui_issue::common::IssueOperation;
use crate::tui_issue::common::Mode;

use super::{Message, State};

type Widget = widget::Widget<State, Message>;

#[derive(Clone, Default)]
struct BrowserProps<'a> {
    /// Application mode: openation and id or id only.
    mode: Mode,
    /// Filtered issues.
    issues: Vec<IssueItem>,
    /// Current (selected) table index
    selected: Option<usize>,
    /// Issue statistics.
    stats: HashMap<String, usize>,
    /// Header columns
    header: Vec<Column<'a>>,
    /// Table columns
    columns: Vec<Column<'a>>,
    /// Max. width, before columns are cut-off.
    cutoff: usize,
    /// Column index that marks where to cut.
    cutoff_after: usize,
    /// Current page size (height of table content).
    page_size: usize,
    /// If search widget should be shown.
    show_search: bool,
    /// Current search string.
    search: String,
}

impl<'a> From<&State> for BrowserProps<'a> {
    fn from(state: &State) -> Self {
        use radicle::issue::State;

        let issues = state.browser.issues();

        let mut open = 0;
        let mut other = 0;
        let mut solved = 0;

        for issue in &issues {
            match issue.state {
                State::Open => open += 1,
                State::Closed {
                    reason: CloseReason::Other,
                } => other += 1,
                State::Closed {
                    reason: CloseReason::Solved,
                } => solved += 1,
            }
        }

        let closed = solved + other;

        let stats = HashMap::from([
            ("Open".to_string(), open),
            ("Other".to_string(), other),
            ("Solved".to_string(), solved),
            ("Closed".to_string(), closed),
        ]);

        Self {
            mode: state.mode.clone(),
            issues,
            selected: state.browser.selected,
            stats,
            header: [
                Column::new(" ● ", Constraint::Length(3)),
                Column::new("ID", Constraint::Length(8)),
                Column::new("Title", Constraint::Fill(5)),
                Column::new("Author", Constraint::Length(16)),
                Column::new("", Constraint::Length(16)),
                Column::new("Labels", Constraint::Fill(1)),
                Column::new("Assignees", Constraint::Fill(1)),
                Column::new("Opened", Constraint::Length(16)),
            ]
            .to_vec(),
            columns: [
                Column::new(" ● ", Constraint::Length(3)),
                Column::new("ID", Constraint::Length(8)),
                Column::new("Title", Constraint::Fill(5)),
                Column::new("Author", Constraint::Length(16)),
                Column::new("", Constraint::Length(16)),
                Column::new("Labels", Constraint::Fill(1)),
                Column::new("Assignees", Constraint::Fill(1)),
                Column::new("Opened", Constraint::Length(16)),
            ]
            .to_vec(),
            cutoff: 200,
            cutoff_after: 5,
            page_size: state.browser.page_size,
            search: state.browser.search.read(),
            show_search: state.browser.show_search,
        }
    }
}

pub struct Browser {
    /// Notifications widget
    issues: Widget,
    /// Search widget
    search: Widget,
}

impl Browser {
    fn new(tx: UnboundedSender<Message>) -> Self {
        Self {
            issues: Container::default()
                .header(
                    Header::default()
                        // .columns(props.header.clone())
                        // .cutoff(props.cutoff, props.cutoff_after)
                        .to_widget(tx.clone()),
                )
                .content(
                    Table::<State, Message, IssueItem, 8>::default()
                        .to_widget(tx.clone())
                        .on_event(|_, s, _| {
                            Some(Message::Select {
                                selected: s.and_then(|s| s.unwrap_usize()),
                            })
                        })
                        .on_update(|state| {
                            let props = BrowserProps::from(state);

                            TableProps::default()
                                .columns(props.columns)
                                .items(state.browser.issues())
                                .selected(state.browser.selected)
                                .footer(!state.browser.show_search)
                                .page_size(state.browser.page_size)
                                .cutoff(props.cutoff, props.cutoff_after)
                                .to_boxed_any()
                                .into()
                        }),
                )
                .footer(Footer::default().to_widget(tx.clone()).on_update(|state| {
                    let props = BrowserProps::from(state);

                    FooterProps::default()
                        .columns(browse_footer(&props, props.selected))
                        .to_boxed_any()
                        .into()
                }))
                .to_widget(tx.clone())
                .on_update(|state| {
                    ContainerProps::default()
                        .hide_footer(BrowserProps::from(state).show_search)
                        .to_boxed_any()
                        .into()
                }),
            search: Search::new(tx.clone()).to_widget(tx.clone()),
        }
    }
}

impl View for Browser {
    type Message = Message;
    type State = State;

    fn handle_event(&mut self, props: Option<&ViewProps>, key: Key) -> Option<Self::Message> {
        let default = BrowserProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<BrowserProps>())
            .unwrap_or(&default);

        if props.show_search {
            self.search.handle_event(key);
            None
        } else {
            match key {
                Key::Char('/') => Some(Message::OpenSearch),
                Key::Char('\n') => {
                    let operation = match props.mode {
                        Mode::Operation => Some(IssueOperation::Show.to_string()),
                        Mode::Id => None,
                    };

                    props
                        .selected
                        .and_then(|selected| props.issues.get(selected))
                        .map(|issue| Message::Exit {
                            selection: Some(Selection {
                                operation,
                                ids: vec![issue.id],
                                args: vec![],
                            }),
                        })
                }
                Key::Char('e') => props
                    .selected
                    .and_then(|selected| props.issues.get(selected))
                    .map(|issue| Message::Exit {
                        selection: Some(Selection {
                            operation: Some(IssueOperation::Edit.to_string()),
                            ids: vec![issue.id],
                            args: vec![],
                        }),
                    }),
                _ => {
                    self.issues.handle_event(key);
                    None
                }
            }
        }
    }

    fn update(&mut self, _props: Option<&ViewProps>, state: &Self::State) {
        self.issues.update(state);
        self.search.update(state);
    }

    fn render(&self, props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let default = BrowserProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<BrowserProps>())
            .unwrap_or(&default);

        if props.show_search {
            let [table_area, search_area] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(render.area);

            self.issues.render(RenderProps::from(table_area), frame);
            self.search
                .render(RenderProps::from(search_area).focus(render.focus), frame);
        } else {
            self.issues.render(render, frame);
        }
    }
}

#[derive(Clone, Default)]
struct BrowserPageProps<'a> {
    /// Current page size (height of table content).
    page_size: usize,
    /// If this pages' keys should be handled (`false` if search is shown).
    handle_keys: bool,
    /// This pages' shortcuts.
    shortcuts: Vec<(&'a str, &'a str)>,
}

impl<'a> From<&State> for BrowserPageProps<'a> {
    fn from(state: &State) -> Self {
        Self {
            page_size: state.browser.page_size,
            handle_keys: !state.browser.show_search,
            shortcuts: if state.browser.show_search {
                vec![("esc", "cancel"), ("enter", "apply")]
            } else {
                match state.mode {
                    Mode::Id => vec![("enter", "select"), ("/", "search")],
                    Mode::Operation => vec![
                        ("enter", "show"),
                        ("e", "edit"),
                        ("/", "search"),
                        ("?", "help"),
                    ],
                }
            },
        }
    }
}

pub struct BrowserPage<'a> {
    /// Internal props
    props: BrowserPageProps<'a>,
    /// Sections widget
    sections: Widget,
    /// Shortcut widget
    shortcuts: Widget,
}

impl<'a: 'static> BrowserPage<'a> {
    pub fn new(tx: UnboundedSender<Message>) -> Self {
        Self {
            props: BrowserPageProps::default(),
            sections: SectionGroup::default()
                .section(Browser::new(tx.clone()).to_widget(tx.clone()))
                .to_widget(tx.clone())
                .on_update(|state| {
                    let props = BrowserPageProps::from(state);
                    SectionGroupProps::default()
                        .handle_keys(props.handle_keys)
                        .to_boxed_any()
                        .into()
                }),
            shortcuts: Shortcuts::default()
                .to_widget(tx.clone())
                .on_update(|state| {
                    ShortcutsProps::default()
                        .shortcuts(&BrowserPageProps::from(state).shortcuts)
                        .to_boxed_any()
                        .into()
                }),
        }
    }
}

impl<'a: 'static> View for BrowserPage<'a> {
    type Message = Message;
    type State = State;

    fn handle_event(&mut self, _props: Option<&ViewProps>, key: Key) -> Option<Self::Message> {
        self.sections.handle_event(key);

        if self.props.handle_keys {
            return match key {
                Key::Esc | Key::Ctrl('c') => Some(Message::Exit { selection: None }),
                Key::Char('?') => Some(Message::OpenHelp),
                _ => None,
            };
        }

        None
    }

    fn update(&mut self, _props: Option<&ViewProps>, state: &Self::State) {
        self.sections.update(state);
        self.shortcuts.update(state);
    }

    fn render(&self, _props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let page_size = render.area.height.saturating_sub(6) as usize;

        let [content_area, shortcuts_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(render.area);

        self.sections.render(
            RenderProps::from(content_area)
                .layout(Layout::horizontal([Constraint::Min(1)]))
                .focus(true),
            frame,
        );
        self.shortcuts
            .render(RenderProps::from(shortcuts_area), frame);

        // TODO: Find better solution
        if page_size != self.props.page_size {
            self.sections.send(Message::BrowserPageSize(page_size));
        }
    }
}

#[derive(Clone)]
pub struct SearchProps {}

pub struct Search {
    /// Internal props
    props: SearchProps,
    /// Search input field
    input: Widget,
}

impl Search {
    fn new(tx: UnboundedSender<Message>) -> Self
    where
        Self: Sized,
    {
        Self {
            props: SearchProps {},
            input: TextField::default()
                .to_widget(tx.clone())
                .on_event(|_, s, _| {
                    Some(Message::UpdateSearch {
                        value: s.and_then(|i| i.unwrap_string()).unwrap_or_default(),
                    })
                })
                .on_update(|state: &State| {
                    TextFieldProps::default()
                        .text(&state.browser.search.read().to_string())
                        .title("Search")
                        .inline(true)
                        .to_boxed_any()
                        .into()
                }),
        }
    }
}

impl View for Search {
    type Message = Message;
    type State = State;

    fn handle_event(&mut self, _props: Option<&ViewProps>, key: Key) -> Option<Self::Message> {
        match key {
            Key::Esc => Some(Message::CloseSearch),
            Key::Char('\n') => Some(Message::ApplySearch),
            _ => {
                self.input.handle_event(key);
                None
            }
        }
    }

    fn update(&mut self, _props: Option<&ViewProps>, state: &Self::State) {
        self.input.update(state);
    }

    fn render(&self, _props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let layout = Layout::horizontal(Constraint::from_mins([0]))
            .horizontal_margin(1)
            .split(render.area);

        self.input.render(RenderProps::from(layout[0]), frame);
    }
}

#[derive(Clone, Default)]
struct HelpPageProps<'a> {
    /// Current page size (height of table content).
    page_size: usize,
    /// Scroll progress of help paragraph.
    help_progress: usize,
    /// This pages' shortcuts.
    shortcuts: Vec<(&'a str, &'a str)>,
}

impl<'a> From<&State> for HelpPageProps<'a> {
    fn from(state: &State) -> Self {
        Self {
            page_size: state.help.page_size,
            help_progress: state.help.progress,
            shortcuts: vec![("?", "close")],
        }
    }
}

pub struct HelpPage {
    /// Content widget
    content: Widget,
    /// Shortcut widget
    shortcuts: Widget,
}

impl HelpPage {
    pub fn new(tx: UnboundedSender<Message>) -> Self
    where
        Self: Sized,
    {
        Self {
            content: Container::default()
                .header(Header::default().to_widget(tx.clone()).on_update(|_| {
                    HeaderProps::default()
                        .columns([Column::new(" Help ", Constraint::Fill(1))].to_vec())
                        .to_boxed_any()
                        .into()
                }))
                .content(
                    Paragraph::default()
                        .to_widget(tx.clone())
                        .on_event(|_, s, _| {
                            Some(Message::ScrollHelp {
                                progress: s.and_then(|p| p.unwrap_usize()).unwrap_or_default(),
                            })
                        })
                        .on_update(|state| {
                            let props = HelpPageProps::from(state);

                            ParagraphProps::default()
                                .text(&help_text())
                                .page_size(props.page_size)
                                .to_boxed_any()
                                .into()
                        }),
                )
                .footer(Footer::default().to_widget(tx.clone()).on_update(|state| {
                    let props = HelpPageProps::from(state);

                    FooterProps::default()
                        .columns(
                            [
                                Column::new(Text::raw(""), Constraint::Fill(1)),
                                Column::new(
                                    span::default(&format!("{}%", props.help_progress)).dim(),
                                    Constraint::Min(4),
                                ),
                            ]
                            .to_vec(),
                        )
                        .to_boxed_any()
                        .into()
                }))
                .to_widget(tx.clone()),
            shortcuts: Shortcuts::default()
                .to_widget(tx.clone())
                .on_update(|state| {
                    ShortcutsProps::default()
                        .shortcuts(&HelpPageProps::from(state).shortcuts)
                        .to_boxed_any()
                        .into()
                }),
        }
    }
}

impl View for HelpPage {
    type Message = Message;
    type State = State;

    fn handle_event(&mut self, _props: Option<&ViewProps>, key: Key) -> Option<Self::Message> {
        match key {
            Key::Esc | Key::Ctrl('c') => Some(Message::Exit { selection: None }),
            Key::Char('?') => Some(Message::LeavePage),
            _ => {
                self.content.handle_event(key);
                None
            }
        }
    }

    fn update(&mut self, _props: Option<&ViewProps>, state: &Self::State) {
        self.content.update(state);
        self.shortcuts.update(state);
    }

    fn render(&self, props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let default = HelpPageProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<HelpPageProps>())
            .unwrap_or(&default);

        let page_size = render.area.height.saturating_sub(6) as usize;

        let [content_area, shortcuts_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(render.area);

        self.content
            .render(RenderProps::from(content_area).focus(true), frame);
        self.shortcuts
            .render(RenderProps::from(content_area).focus(true), frame);

        // TODO: Find better solution
        if page_size != props.page_size {
            self.content.send(Message::HelpPageSize(page_size));
        }
    }
}

fn browse_footer<'a>(props: &BrowserProps<'a>, selected: Option<usize>) -> Vec<Column<'a>> {
    let search = Line::from(vec![
        span::default(" Search ").cyan().dim().reversed(),
        span::default(" "),
        span::default(&props.search).gray().dim(),
    ]);

    let open = Line::from(vec![
        span::positive(&props.stats.get("Open").unwrap_or(&0).to_string()).dim(),
        span::default(" Open").dim(),
    ]);
    let solved = Line::from(vec![
        span::default(&props.stats.get("Solved").unwrap_or(&0).to_string())
            .magenta()
            .dim(),
        span::default(" Solved").dim(),
    ]);
    let closed = Line::from(vec![
        span::default(&props.stats.get("Closed").unwrap_or(&0).to_string())
            .magenta()
            .dim(),
        span::default(" Closed").dim(),
    ]);
    let sum = Line::from(vec![
        span::default("Σ ").dim(),
        span::default(&props.issues.len().to_string()).dim(),
    ]);

    let progress = selected
        .map(|selected| TableUtils::progress(selected, props.issues.len(), props.page_size))
        .unwrap_or_default();
    let progress = span::default(&format!("{}%", progress)).dim();

    match IssueItemFilter::from_str(&props.search)
        .unwrap_or_default()
        .state()
    {
        Some(state) => {
            let block = match state {
                issue::State::Open => open,
                issue::State::Closed {
                    reason: issue::CloseReason::Other,
                } => closed,
                issue::State::Closed {
                    reason: issue::CloseReason::Solved,
                } => solved,
            };

            [
                Column::new(Text::from(search), Constraint::Fill(1)),
                Column::new(
                    Text::from(block.clone()),
                    Constraint::Min(block.width() as u16),
                ),
                Column::new(Text::from(progress), Constraint::Min(4)),
            ]
            .to_vec()
        }
        None => [
            Column::new(Text::from(search), Constraint::Fill(1)),
            Column::new(
                Text::from(open.clone()),
                Constraint::Min(open.width() as u16),
            ),
            Column::new(
                Text::from(closed.clone()),
                Constraint::Min(closed.width() as u16),
            ),
            Column::new(Text::from(sum.clone()), Constraint::Min(sum.width() as u16)),
            Column::new(Text::from(progress), Constraint::Min(4)),
        ]
        .to_vec(),
    }
}

fn help_text() -> Text<'static> {
    Text::from(
        [
            Line::from(Span::raw("Generic keybindings").cyan()),
            Line::raw(""),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "↑,k")).gray(),
                    Span::raw(" "),
                    Span::raw("move cursor one line up").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "↓,j")).gray(),
                    Span::raw(" "),
                    Span::raw("move cursor one line down").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "PageUp")).gray(),
                    Span::raw(" "),
                    Span::raw("move cursor one page up").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "PageDown")).gray(),
                    Span::raw(" "),
                    Span::raw("move cursor one page down").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "Home")).gray(),
                    Span::raw(" "),
                    Span::raw("move cursor to the first line").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "End")).gray(),
                    Span::raw(" "),
                    Span::raw("move cursor to the last line").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::raw(""),
            Line::from(Span::raw("Specific keybindings").cyan()),
            Line::raw(""),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "enter")).gray(),
                    Span::raw(" "),
                    Span::raw("Select issue (if --mode id)").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "enter")).gray(),
                    Span::raw(" "),
                    Span::raw("Show issue").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "e")).gray(),
                    Span::raw(" "),
                    Span::raw("Edit patch").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "/")).gray(),
                    Span::raw(" "),
                    Span::raw("Search").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "?")).gray(),
                    Span::raw(" "),
                    Span::raw("Show help").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "Esc")).gray(),
                    Span::raw(" "),
                    Span::raw("Quit / cancel").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::raw(""),
            Line::from(Span::raw("Searching").cyan()),
            Line::raw(""),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "Pattern")).gray(),
                    Span::raw(" "),
                    Span::raw("is:<state> | is:authored | is:assigned | authors:[<did>, ...] | assignees:[<did>, ...] | <search>")
                        .gray()
                        .dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "Example")).gray(),
                    Span::raw(" "),
                    Span::raw("is:solved is:authored alias").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::raw(""),
            Line::raw(""),
        ]
        .to_vec())
}
