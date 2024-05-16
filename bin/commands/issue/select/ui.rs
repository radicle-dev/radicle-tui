use std::collections::HashMap;
use std::str::FromStr;
use std::vec;

use radicle::issue::{self, CloseReason};
use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::layout::{Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};

use radicle_tui as tui;

use tui::ui::items::{IssueItem, IssueItemFilter};
use tui::ui::span;
use tui::ui::widget;
use tui::ui::widget::container::{
    Column, Container, ContainerProps, Footer, FooterProps, Header, HeaderProps, SectionGroup,
    SectionGroupProps,
};
use tui::ui::widget::input::{TextField, TextFieldProps};
use tui::ui::widget::list::{Table, TableProps, TableUtils};
use tui::ui::widget::text::{Paragraph, ParagraphProps};
use tui::ui::widget::window::{Shortcuts, ShortcutsProps};
use tui::ui::widget::{BoxedAny, Properties, RenderProps, Widget, WidgetBase};

use tui::Selection;

use crate::tui_issue::common::IssueOperation;
use crate::tui_issue::common::Mode;

use super::{Message, State};

type BoxedWidget = widget::BoxedWidget<State, Message>;

#[derive(Clone)]
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

impl<'a> Properties for BrowserProps<'a> {}
impl<'a> BoxedAny for BrowserProps<'a> {}

pub struct Browser<'a> {
    /// Internal base
    base: WidgetBase<State, Message>,
    /// Internal props
    props: BrowserProps<'a>,
    /// Notifications widget
    issues: BoxedWidget,
    /// Search widget
    search: BoxedWidget,
}

impl<'a: 'static> Widget for Browser<'a> {
    type Message = Message;
    type State = State;

    fn new(state: &State, tx: UnboundedSender<Message>) -> Self {
        let props = BrowserProps::from(state);

        Self {
            base: WidgetBase::new(tx.clone()),
            props: BrowserProps::from(state),
            issues: Container::new(state, tx.clone())
                .header(
                    Header::new(state, tx.clone())
                        .columns(props.header.clone())
                        .cutoff(props.cutoff, props.cutoff_after)
                        .to_boxed(),
                )
                .content(Box::<Table<State, Message, IssueItem, 8>>::new(
                    Table::new(state, tx.clone())
                        .on_event(|table| {
                            table
                                .downcast_mut::<Table<State, Message, IssueItem, 8>>()
                                .and_then(|table| {
                                    table
                                        .send(Message::Select {
                                            selected: table.selected(),
                                        })
                                        .ok()
                                });
                        })
                        .on_update(|state| {
                            let props = BrowserProps::from(state);

                            TableProps::default()
                                .columns(props.columns)
                                .items(state.browser.issues())
                                .footer(!state.browser.show_search)
                                .page_size(state.browser.page_size)
                                .cutoff(props.cutoff, props.cutoff_after)
                                .to_boxed()
                        }),
                ))
                .footer(
                    Footer::new(state, tx.clone())
                        .on_update(|state| {
                            let props = BrowserProps::from(state);

                            FooterProps::default()
                                .columns(browse_footer(&props, props.selected))
                                .to_boxed()
                        })
                        .to_boxed(),
                )
                .on_update(|state| {
                    ContainerProps::default()
                        .hide_footer(BrowserProps::from(state).show_search)
                        .to_boxed()
                })
                .to_boxed(),
            search: Search::new(state, tx.clone()).to_boxed(),
        }
    }

    fn handle_event(&mut self, key: Key) {
        if self.props.show_search {
            self.search.handle_event(key);
        } else {
            match key {
                Key::Char('/') => {
                    let _ = self.send(Message::OpenSearch);
                }
                Key::Char('\n') => {
                    let operation = match self.props.mode {
                        Mode::Operation => Some(IssueOperation::Show.to_string()),
                        Mode::Id => None,
                    };

                    self.props
                        .selected
                        .and_then(|selected| self.props.issues.get(selected))
                        .and_then(|issue| {
                            self.base
                                .send(Message::Exit {
                                    selection: Some(Selection {
                                        operation,
                                        ids: vec![issue.id],
                                        args: vec![],
                                    }),
                                })
                                .ok()
                        });
                }
                Key::Char('e') => {
                    self.props
                        .selected
                        .and_then(|selected| self.props.issues.get(selected))
                        .and_then(|issue| {
                            self.base
                                .send(Message::Exit {
                                    selection: Some(Selection {
                                        operation: Some(IssueOperation::Edit.to_string()),
                                        ids: vec![issue.id],
                                        args: vec![],
                                    }),
                                })
                                .ok()
                        });
                }
                _ => {
                    self.issues.handle_event(key);
                }
            }
        }
    }

    fn update(&mut self, state: &State) {
        self.props = BrowserProps::from_callback(self.base.on_update, state)
            .unwrap_or(BrowserProps::from(state));

        self.issues.update(state);
        self.search.update(state);
    }

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        if self.props.show_search {
            let [table_area, search_area] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(props.area);

            self.issues.render(frame, RenderProps::from(table_area));
            self.search
                .render(frame, RenderProps::from(search_area).focus(props.focus));
        } else {
            self.issues.render(frame, props);
        }
    }

    fn base(&self) -> &WidgetBase<State, Message> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut WidgetBase<State, Message> {
        &mut self.base
    }
}

#[derive(Clone)]
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

impl<'a> Properties for BrowserPageProps<'a> {}
impl<'a> BoxedAny for BrowserPageProps<'a> {}

pub struct BrowserPage<'a> {
    /// Internal base
    base: WidgetBase<State, Message>,
    /// Internal props
    props: BrowserPageProps<'a>,
    /// Sections widget
    sections: BoxedWidget,
    /// Shortcut widget
    shortcuts: BoxedWidget,
}

impl<'a: 'static> Widget for BrowserPage<'a> {
    type Message = Message;
    type State = State;

    fn new(state: &State, tx: UnboundedSender<Message>) -> Self {
        let props = BrowserPageProps::from(state);

        Self {
            base: WidgetBase::new(tx.clone()),
            props: props.clone(),
            sections: SectionGroup::new(state, tx.clone())
                .section(Browser::new(state, tx.clone()).to_boxed())
                .on_update(|state| {
                    let props = BrowserPageProps::from(state);
                    SectionGroupProps::default()
                        .handle_keys(props.handle_keys)
                        .to_boxed()
                })
                .to_boxed(),
            shortcuts: Shortcuts::new(state, tx.clone())
                .on_update(|state| {
                    ShortcutsProps::default()
                        .shortcuts(&BrowserPageProps::from(state).shortcuts)
                        .to_boxed()
                })
                .to_boxed(),
        }
    }

    fn handle_event(&mut self, key: Key) {
        self.sections.handle_event(key);

        if self.props.handle_keys {
            match key {
                Key::Esc | Key::Ctrl('c') => {
                    let _ = self.send(Message::Exit { selection: None });
                }
                Key::Char('?') => {
                    let _ = self.send(Message::OpenHelp);
                }
                _ => {}
            }
        }
    }

    fn update(&mut self, state: &State) {
        self.props = BrowserPageProps::from_callback(self.base.on_update, state)
            .unwrap_or(BrowserPageProps::from(state));

        self.sections.update(state);
        self.shortcuts.update(state);
    }

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let page_size = props.area.height.saturating_sub(6) as usize;

        let [content_area, shortcuts_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(props.area);

        self.sections.render(
            frame,
            RenderProps::from(content_area)
                .layout(Layout::horizontal([Constraint::Min(1)]))
                .focus(true),
        );
        self.shortcuts
            .render(frame, RenderProps::from(shortcuts_area));

        if page_size != self.props.page_size {
            let _ = self.send(Message::BrowserPageSize(page_size));
        }
    }

    fn base(&self) -> &WidgetBase<State, Message> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut WidgetBase<State, Message> {
        &mut self.base
    }
}

pub struct SearchProps {}

impl Properties for SearchProps {}

pub struct Search {
    /// Internal base
    base: WidgetBase<State, Message>,
    /// Internal props
    _props: SearchProps,
    /// Search input field
    input: BoxedWidget,
}

impl Widget for Search {
    type Message = Message;
    type State = State;

    fn new(state: &State, tx: UnboundedSender<Message>) -> Self
    where
        Self: Sized,
    {
        Self {
            base: WidgetBase::new(tx.clone()),
            _props: SearchProps {},
            input: TextField::new(state, tx.clone())
                .on_event(|widget| {
                    widget
                        .downcast_mut::<TextField<State, Message>>()
                        .and_then(|field| {
                            field
                                .send(Message::UpdateSearch {
                                    value: field.text().unwrap_or(&String::new()).to_string(),
                                })
                                .ok()
                        });
                })
                .on_update(|state| {
                    TextFieldProps::default()
                        .text(&state.browser.search.read().to_string())
                        .title("Search")
                        .inline(true)
                        .to_boxed()
                })
                .to_boxed(),
        }
    }

    fn handle_event(&mut self, key: termion::event::Key) {
        match key {
            Key::Esc => {
                let _ = self.send(Message::CloseSearch);
            }
            Key::Char('\n') => {
                let _ = self.send(Message::ApplySearch);
            }
            _ => {
                self.input.handle_event(key);
            }
        }
    }

    fn update(&mut self, state: &State) {
        self.input.update(state);
    }

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let layout = Layout::horizontal(Constraint::from_mins([0]))
            .horizontal_margin(1)
            .split(props.area);

        self.input.render(frame, RenderProps::from(layout[0]));
    }

    fn base(&self) -> &WidgetBase<State, Message> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut WidgetBase<State, Message> {
        &mut self.base
    }
}

#[derive(Clone)]
struct HelpPageProps<'a> {
    page_size: usize,
    help_progress: usize,
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

impl<'a> Properties for HelpPageProps<'a> {}
impl<'a> BoxedAny for HelpPageProps<'a> {}

pub struct HelpPage<'a> {
    /// Internal base
    base: WidgetBase<State, Message>,
    /// Internal props
    props: HelpPageProps<'a>,
    /// Content widget
    content: BoxedWidget,
    /// Shortcut widget
    shortcuts: BoxedWidget,
}

impl<'a: 'static> Widget for HelpPage<'a> {
    type Message = Message;
    type State = State;

    fn new(state: &State, tx: UnboundedSender<Message>) -> Self
    where
        Self: Sized,
    {
        Self {
            base: WidgetBase::new(tx.clone()),
            props: HelpPageProps::from(state),
            content: Container::new(state, tx.clone())
                .header(
                    Header::new(state, tx.clone())
                        .on_update(|_| {
                            HeaderProps::default()
                                .columns([Column::new(" Help ", Constraint::Fill(1))].to_vec())
                                .to_boxed()
                        })
                        .to_boxed(),
                )
                .content(
                    Paragraph::new(state, tx.clone())
                        .on_event(|paragraph| {
                            paragraph
                                .downcast_mut::<Paragraph<'_, State, Message>>()
                                .and_then(|paragraph| {
                                    paragraph
                                        .send(Message::ScrollHelp {
                                            progress: paragraph.progress(),
                                        })
                                        .ok()
                                });
                        })
                        .on_update(|state| {
                            let props = HelpPageProps::from(state);

                            ParagraphProps::default()
                                .text(&help_text())
                                .page_size(props.page_size)
                                .to_boxed()
                        })
                        .to_boxed(),
                )
                .footer(
                    Footer::new(state, tx.clone())
                        .on_update(|state| {
                            let props = HelpPageProps::from(state);

                            FooterProps::default()
                                .columns(
                                    [
                                        Column::new(Text::raw(""), Constraint::Fill(1)),
                                        Column::new(
                                            span::default(&format!("{}%", props.help_progress))
                                                .dim(),
                                            Constraint::Min(4),
                                        ),
                                    ]
                                    .to_vec(),
                                )
                                .to_boxed()
                        })
                        .to_boxed(),
                )
                .to_boxed(),
            shortcuts: Shortcuts::new(state, tx.clone())
                .on_update(|state| {
                    ShortcutsProps::default()
                        .shortcuts(&HelpPageProps::from(state).shortcuts)
                        .to_boxed()
                })
                .to_boxed(),
        }
    }

    fn handle_event(&mut self, key: termion::event::Key) {
        match key {
            Key::Esc | Key::Ctrl('c') => {
                let _ = self.send(Message::Exit { selection: None });
            }
            Key::Char('?') => {
                let _ = self.send(Message::LeavePage);
            }
            _ => {
                self.content.handle_event(key);
            }
        }
    }

    fn update(&mut self, state: &State) {
        self.props = HelpPageProps::from_callback(self.base.on_update, state)
            .unwrap_or(HelpPageProps::from(state));

        self.content.update(state);
        self.shortcuts.update(state);
    }

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let page_size = props.area.height.saturating_sub(6) as usize;

        let [content_area, shortcuts_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(props.area);

        self.content
            .render(frame, RenderProps::from(content_area).focus(true));
        self.shortcuts
            .render(frame, RenderProps::from(shortcuts_area));

        if page_size != self.props.page_size {
            let _ = self.send(Message::HelpPageSize(page_size));
        }
    }

    fn base(&self) -> &WidgetBase<State, Message> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut WidgetBase<State, Message> {
        &mut self.base
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
