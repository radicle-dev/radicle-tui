#[path = "select/ui.rs"]
mod ui;

use std::collections::HashMap;
use std::str::FromStr;

use anyhow::Result;

use radicle::cob::thread::CommentId;
use radicle::git::Oid;
use termion::event::Key;

use ratatui::layout::Constraint;
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};

use radicle::issue::IssueId;
use radicle::storage::git::Repository;
use radicle::Profile;

use radicle_tui as tui;

use tui::store;
use tui::store::StateValue;
use tui::ui::span;
use tui::ui::widget::container::{
    Column, Container, Footer, FooterProps, Header, HeaderProps, SectionGroup, SectionGroupProps,
    SplitContainer, SplitContainerFocus, SplitContainerProps,
};
use tui::ui::widget::input::{TextView, TextViewProps};
use tui::ui::widget::list::{Tree, TreeProps};
use tui::ui::widget::window::{Page, PageProps, Shortcuts, ShortcutsProps, Window, WindowProps};
use tui::ui::widget::{PredefinedLayout, ToWidget, Widget};

use tui::{BoxedAny, Channel, Exit, PageStack};

use crate::cob::issue;
use crate::ui::items::{CommentItem, Filter, IssueItem, IssueItemFilter};
use crate::ui::widget::{IssueDetails, IssueDetailsProps};

use self::ui::{Browser, BrowserProps};

use super::common::Mode;

type Selection = tui::Selection<IssueId>;

pub struct Context {
    pub profile: Profile,
    pub repository: Repository,
    pub mode: Mode,
    pub filter: issue::Filter,
}

pub struct App {
    context: Context,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum AppPage {
    Browser,
    Help,
}

#[derive(Clone, Debug)]
pub struct BrowserState {
    items: Vec<IssueItem>,
    scroll: usize,
    selected: Option<usize>,
    filter: IssueItemFilter,
    search: store::StateValue<String>,
    show_search: bool,
}

impl BrowserState {
    pub fn issues(&self) -> Vec<IssueItem> {
        self.items
            .iter()
            .filter(|patch| self.filter.matches(patch))
            .cloned()
            .collect()
    }
}

impl BrowserState {
    pub fn show_search(&mut self) {
        self.show_search = true;
    }

    pub fn hide_search(&mut self) {
        self.show_search = false;
    }

    pub fn apply_search(&mut self) {
        self.search.apply();
    }

    pub fn reset_search(&mut self) {
        self.search.reset();
    }

    pub fn reset_scroll(&mut self) {
        self.scroll = 0;
    }

    pub fn scroll(&mut self, scroll: usize) {
        self.scroll = scroll;
    }

    pub fn search(&mut self, value: String) {
        self.search.write(value);
        self.filter_items();
    }

    pub fn filter_items(&mut self) {
        self.filter = IssueItemFilter::from_str(&self.search.read()).unwrap_or_default();
    }
}

#[derive(Clone, Debug)]
pub struct CommentState {
    /// Current text view cursor.
    cursor: (usize, usize),
}

impl CommentState {
    pub fn reset_cursor(&mut self) {
        self.cursor = (0, 0);
    }

    pub fn update_cursor(&mut self, cursor: (usize, usize)) {
        self.cursor = cursor;
    }
}

#[derive(Clone, Debug)]
pub struct IssueState {
    /// Currently selected issue item.
    item: Option<IssueItem>,
    /// Tree selection per issue.
    selected_comments: HashMap<IssueId, Vec<CommentId>>,
    /// State of currently selected comment
    comment: CommentState,
}

#[derive(Clone, Debug)]
pub struct HelpState {
    scroll: usize,
    cursor: (usize, usize),
}

#[derive(Clone, Debug)]
pub struct State {
    mode: Mode,
    pages: PageStack<AppPage>,
    browser: BrowserState,
    issue: IssueState,
    help: HelpState,
}

impl TryFrom<&Context> for State {
    type Error = anyhow::Error;

    fn try_from(context: &Context) -> Result<Self, Self::Error> {
        let issues = issue::all(&context.profile, &context.repository)?;
        let search = StateValue::new(context.filter.to_string());
        let filter = IssueItemFilter::from_str(&search.read()).unwrap_or_default();

        // Convert into UI items
        let mut items = vec![];
        for issue in issues {
            if let Ok(item) = IssueItem::new(&context.profile, issue.clone()) {
                items.push(item);
            }
        }
        items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Pre-select first comment
        let mut selected_comments = HashMap::new();
        for item in &items {
            selected_comments.insert(
                item.id,
                item.root_comments()
                    .first()
                    .and_then(|comment| Some(vec![comment.id]))
                    .unwrap_or_default(),
            );
        }

        Ok(Self {
            mode: context.mode.clone(),
            pages: PageStack::new(vec![AppPage::Browser]),
            browser: BrowserState {
                items: items.clone(),
                selected: Some(0),
                scroll: 0,
                filter,
                search,
                show_search: false,
            },
            issue: IssueState {
                item: items.get(0).cloned(),
                selected_comments,
                comment: CommentState { cursor: (0, 0) },
            },
            help: HelpState {
                scroll: 0,
                cursor: (0, 0),
            },
        })
    }
}

pub enum Message {
    Exit {
        selection: Option<Selection>,
    },
    SelectIssue {
        selected: Option<usize>,
        scroll: usize,
    },
    OpenSearch,
    UpdateSearch {
        value: String,
    },
    ApplySearch,
    CloseSearch,
    SelectComment {
        selected: Option<Vec<CommentId>>,
    },
    ScrollComment {
        cursor: (usize, usize),
    },
    OpenHelp,
    LeavePage,
    ScrollHelp {
        scroll: usize,
        cursor: (usize, usize),
    },
}

impl store::State<Selection> for State {
    type Message = Message;

    fn update(&mut self, message: Message) -> Option<Exit<Selection>> {
        match message {
            Message::Exit { selection } => Some(Exit { value: selection }),
            Message::SelectIssue { selected, scroll } => {
                self.browser.selected = selected;
                self.browser.scroll(scroll);

                self.issue.item = self
                    .browser
                    .selected
                    .and_then(|selected| self.browser.issues().get(selected).cloned());
                self.issue.comment.reset_cursor();

                None
            }
            Message::SelectComment { selected } => {
                if let Some(item) = &self.issue.item {
                    self.issue
                        .selected_comments
                        .insert(item.id, selected.unwrap_or(vec![]));
                }
                self.issue.comment.reset_cursor();
                None
            }
            Message::ScrollComment { cursor } => {
                self.issue.comment.update_cursor(cursor);
                None
            }
            Message::OpenSearch => {
                self.browser.show_search();
                None
            }
            Message::UpdateSearch { value } => {
                self.browser.search(value);

                if let Some(selected) = self.browser.selected {
                    if selected > self.browser.issues().len() {
                        self.browser.selected = Some(0);
                        self.issue.item = self.browser.issues().get(0).cloned();
                    } else {
                        self.issue.item = self.browser.issues().get(selected).cloned();
                    }
                } else {
                    self.issue.item = None;
                }

                self.browser.reset_scroll();
                None
            }
            Message::ApplySearch => {
                self.browser.hide_search();
                self.browser.apply_search();
                self.browser.reset_scroll();
                None
            }
            Message::CloseSearch => {
                self.browser.hide_search();
                self.browser.reset_search();
                self.browser.filter_items();
                None
            }
            Message::OpenHelp => {
                self.pages.push(AppPage::Help);
                None
            }
            Message::LeavePage => {
                self.pages.pop();
                None
            }
            Message::ScrollHelp { scroll, cursor } => {
                self.help.scroll = scroll;
                self.help.cursor = cursor;
                None
            }
        }
    }
}

impl App {
    pub fn new(context: Context) -> Self {
        Self { context }
    }

    pub async fn run(&self) -> Result<Option<Selection>> {
        let channel = Channel::default();
        let state = State::try_from(&self.context)?;
        let tx = channel.tx.clone();

        let window = Window::default()
            .page(AppPage::Browser, browser_page(&state, &channel))
            .page(AppPage::Help, help_page(&state, &channel))
            .to_widget(tx.clone())
            .on_update(|state| {
                WindowProps::default()
                    .current_page(state.pages.peek().unwrap_or(&AppPage::Browser).clone())
                    .to_boxed_any()
                    .into()
            });

        tui::run(channel, state, window).await
    }
}

fn browser_page(_state: &State, channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    let shortcuts = Shortcuts::default()
        .to_widget(tx.clone())
        .on_update(|state: &State| {
            let shortcuts = if state.browser.show_search {
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
            };

            ShortcutsProps::default()
                .shortcuts(&shortcuts)
                .to_boxed_any()
                .into()
        });

    Page::default()
        .content(
            SectionGroup::default()
                .section(browser(channel))
                .section(issue_details(channel))
                .section(comment(channel))
                .to_widget(tx.clone())
                .on_update(|_| {
                    SectionGroupProps::default()
                        .handle_keys(true)
                        .layout(PredefinedLayout::Expandable3)
                        .to_boxed_any()
                        .into()
                }),
        )
        .shortcuts(shortcuts)
        .to_widget(tx.clone())
        .on_event(|key, _, props| {
            let default = PageProps::default();
            let props = props
                .and_then(|props| props.inner_ref::<PageProps>())
                .unwrap_or(&default);

            if props.handle_keys {
                match key {
                    Key::Esc | Key::Ctrl('c') => Some(Message::Exit { selection: None }),
                    Key::Char('?') => Some(Message::OpenHelp),
                    _ => None,
                }
            } else {
                None
            }
        })
        .on_update(|state: &State| {
            PageProps::default()
                .handle_keys(!state.browser.show_search)
                .to_boxed_any()
                .into()
        })
}

fn help_page(_state: &State, channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    let content = Container::default()
        .header(Header::default().to_widget(tx.clone()).on_update(|_| {
            HeaderProps::default()
                .columns([Column::new(" Help ", Constraint::Fill(1))].to_vec())
                .to_boxed_any()
                .into()
        }))
        .content(
            TextView::default()
                .to_widget(tx.clone())
                .on_event(|_, view_state, _| {
                    view_state
                        .and_then(|tv| tv.unwrap_textview())
                        .map(|tvs| Message::ScrollHelp {
                            scroll: tvs.scroll,
                            cursor: tvs.cursor,
                        })
                })
                .on_update(|state: &State| {
                    TextViewProps::default()
                        .content(help_text())
                        .cursor(state.help.cursor)
                        .to_boxed_any()
                        .into()
                }),
        )
        .footer(
            Footer::default()
                .to_widget(tx.clone())
                .on_update(|state: &State| {
                    FooterProps::default()
                        .columns(
                            [
                                Column::new(Text::raw(""), Constraint::Fill(1)),
                                Column::new(
                                    span::default(&format!("{}%", state.help.scroll)).dim(),
                                    Constraint::Min(4),
                                ),
                            ]
                            .to_vec(),
                        )
                        .to_boxed_any()
                        .into()
                }),
        )
        .to_widget(tx.clone());

    let shortcuts = Shortcuts::default().to_widget(tx.clone()).on_update(|_| {
        ShortcutsProps::default()
            .shortcuts(&[("?", "close")])
            .to_boxed_any()
            .into()
    });

    Page::default()
        .content(content)
        .shortcuts(shortcuts)
        .to_widget(tx.clone())
        .on_event(|key, _, _| match key {
            Key::Esc | Key::Ctrl('c') => Some(Message::Exit { selection: None }),
            Key::Char('?') => Some(Message::LeavePage),
            _ => None,
        })
        .on_update(|_| PageProps::default().handle_keys(true).to_boxed_any().into())
}

fn browser(channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    Browser::new(tx.clone())
        .to_widget(tx.clone())
        .on_update(|state| BrowserProps::from(state).to_boxed_any().into())
}

fn issue_details(channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    SplitContainer::default()
        .top(
            IssueDetails::default()
                .to_widget(tx.clone())
                .on_update(|state: &State| {
                    IssueDetailsProps::default()
                        .issue(state.issue.item.clone())
                        .to_boxed_any()
                        .into()
                }),
        )
        .bottom(
            Tree::<State, Message, CommentItem>::default()
                .to_widget(tx.clone())
                .on_event(|_, s, _| {
                    Some(Message::SelectComment {
                        selected: s.and_then(|s| {
                            s.unwrap_tree().and_then(|tree| {
                                Some(tree.iter().map(|id| Oid::from_str(id).unwrap()).collect())
                            })
                        }),
                    })
                })
                .on_update(|state| {
                    let comments = &state
                        .issue
                        .item
                        .as_ref()
                        .and_then(|item| Some(item.root_comments()))
                        .unwrap_or(vec![]);

                    let selected = &state
                        .issue
                        .item
                        .as_ref()
                        .and_then(|item| state.issue.selected_comments.get(&item.id))
                        .and_then(|selected| {
                            Some(selected.iter().map(|oid| oid.to_string()).collect())
                        })
                        .unwrap_or(vec![]);

                    TreeProps::<CommentItem>::default()
                        .items(comments.to_vec())
                        .selected(selected)
                        .to_boxed_any()
                        .into()
                }),
        )
        .to_widget(tx.clone())
        .on_update(|_| {
            SplitContainerProps::default()
                .heights([Constraint::Length(5), Constraint::Min(1)])
                .split_focus(SplitContainerFocus::Bottom)
                .to_boxed_any()
                .into()
        })
}

fn comment(channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    Container::default()
        .content(
            TextView::default()
                .to_widget(tx.clone())
                .on_event(|_, vs, _| {
                    let textview = vs.and_then(|p| p.unwrap_textview()).unwrap_or_default();
                    Some(Message::ScrollComment {
                        cursor: textview.cursor,
                    })
                })
                .on_update(|state: &State| {
                    let body: String = state
                        .issue
                        .item
                        .as_ref()
                        .and_then(|item| {
                            state
                                .issue
                                .selected_comments
                                .get(&item.id)
                                .and_then(|selection| selection.last().copied())
                                .and_then(|comment_id| {
                                    state.issue.item.as_ref().and_then(|item| {
                                        item.comments
                                            .iter()
                                            .filter(|item| item.id == comment_id)
                                            .collect::<Vec<_>>()
                                            .first()
                                            .cloned()
                                    })
                                })
                                .and_then(|comment| Some(comment.body.clone()))
                        })
                        .unwrap_or_default();

                    TextViewProps::default()
                        .content(body)
                        .cursor(state.issue.comment.cursor)
                        .show_scroll_progress(true)
                        .to_boxed_any()
                        .into()
                }),
        )
        .to_widget(tx.clone())
}

fn help_text() -> Text<'static> {
    Text::from(
        [
            Line::from(Span::raw("Generic keybindings").cyan()),
            Line::raw(""),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "↑,k")).gray(),
                    Span::raw(": "),
                    Span::raw("move cursor one line up").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "↓,j")).gray(),
                    Span::raw(": "),
                    Span::raw("move cursor one line down").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "PageUp")).gray(),
                    Span::raw(": "),
                    Span::raw("move cursor one page up").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "PageDown")).gray(),
                    Span::raw(": "),
                    Span::raw("move cursor one page down").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "Home")).gray(),
                    Span::raw(": "),
                    Span::raw("move cursor to the first line").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "End")).gray(),
                    Span::raw(": "),
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
                    Span::raw(": "),
                    Span::raw("Select issue (if --mode id)").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "enter")).gray(),
                    Span::raw(": "),
                    Span::raw("Show issue").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "e")).gray(),
                    Span::raw(": "),
                    Span::raw("Edit patch").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "/")).gray(),
                    Span::raw(": "),
                    Span::raw("Search").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "?")).gray(),
                    Span::raw(": "),
                    Span::raw("Show help").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "Esc")).gray(),
                    Span::raw(": "),
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
                    Span::raw(": "),
                    Span::raw("is:<state> | is:authored | is:assigned | authors:[<did>, ...] | assignees:[<did>, ...] | <search>")
                        .gray()
                        .dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "Example")).gray(),
                    Span::raw(": "),
                    Span::raw("is:solved is:authored alias").gray().dim(),
                ]
                .to_vec(),
            ),
        ]
        .to_vec())
}
