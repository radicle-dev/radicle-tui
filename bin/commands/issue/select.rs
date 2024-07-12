#[path = "select/ui.rs"]
mod ui;

use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use anyhow::{bail, Result};

use termion::event::Key;

use ratatui::layout::Constraint;
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};

use radicle::cob::thread::CommentId;
use radicle::git::Oid;
use radicle::issue::IssueId;
use radicle::storage::git::Repository;
use radicle::Profile;

use radicle_tui as tui;

use tui::store;
use tui::store::StateValue;
use tui::ui::span;
use tui::ui::widget::container::{
    Column, Container, ContainerProps, Footer, FooterProps, Header, HeaderProps, SectionGroup,
    SectionGroupProps, SplitContainer, SplitContainerFocus, SplitContainerProps,
};
use tui::ui::widget::input::{TextView, TextViewProps};
use tui::ui::widget::list::{Tree, TreeProps};
use tui::ui::widget::window::{Page, PageProps, Shortcuts, ShortcutsProps, Window, WindowProps};
use tui::ui::widget::{PredefinedLayout, ToWidget, Widget};
use tui::{BoxedAny, Channel, Exit, PageStack};

use crate::cob::issue;
use crate::settings;
use crate::ui::items::{CommentItem, Filter, IssueItem, IssueItemFilter};
use crate::ui::widget::{IssueDetails, IssueDetailsProps};
use crate::ui::TerminalInfo;

use self::ui::{Browser, BrowserProps};

use super::common::{IssueOperation, Mode};

type Selection = tui::Selection<IssueId>;

pub struct Context {
    pub profile: Profile,
    pub repository: Repository,
    pub mode: Mode,
    pub filter: issue::Filter,
    pub terminal_info: TerminalInfo,
}

pub struct App {
    context: Context,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum AppPage {
    Browser,
    Help,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub enum Section {
    #[default]
    Browser,
    Details,
    Comment,
}

impl TryFrom<usize> for Section {
    type Error = anyhow::Error;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Section::Browser),
            1 => Ok(Section::Details),
            2 => Ok(Section::Comment),
            _ => bail!("Unknown section index: {}", value),
        }
    }
}

impl From<Section> for usize {
    fn from(section: Section) -> Self {
        match section {
            Section::Browser => 0,
            Section::Details => 1,
            Section::Comment => 2,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BrowserState {
    items: Vec<IssueItem>,
    selected: Option<usize>,
    filter: IssueItemFilter,
    search: store::StateValue<String>,
    show_search: bool,
}

impl BrowserState {
    pub fn issues(&self) -> Vec<IssueItem> {
        self.issues_ref().into_iter().cloned().collect()
    }

    pub fn issues_ref(&self) -> Vec<&IssueItem> {
        self.items
            .iter()
            .filter(|patch| self.filter.matches(patch))
            .collect()
    }

    pub fn selected_issue(&self) -> Option<&IssueItem> {
        self.selected
            .and_then(|selected| self.issues_ref().get(selected).copied())
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
pub struct PreviewState {
    /// If preview is visible.
    show: bool,
    /// Currently selected issue item.
    issue: Option<IssueItem>,
    /// Tree selection per issue.
    selected_comments: HashMap<IssueId, Vec<CommentId>>,
    /// State of currently selected comment
    comment: CommentState,
}

impl PreviewState {
    pub fn root_comments(&self) -> Vec<CommentItem> {
        self.issue
            .as_ref()
            .map(|item| item.root_comments())
            .unwrap_or_default()
    }

    pub fn selected_comment(&self) -> Option<&CommentItem> {
        self.issue.as_ref().and_then(|item| {
            self.selected_comments
                .get(&item.id)
                .and_then(|selection| selection.last().copied())
                .and_then(|comment_id| {
                    item.comments
                        .iter()
                        .filter(|item| item.id == comment_id)
                        .collect::<Vec<_>>()
                        .first()
                        .cloned()
                })
        })
    }

    pub fn selected_comment_ids(&self) -> Vec<String> {
        self.issue
            .as_ref()
            .and_then(|item| self.selected_comments.get(&item.id))
            .map(|selected| selected.iter().map(|oid| oid.to_string()).collect())
            .unwrap_or_default()
    }

    pub fn opened_comments(&self) -> HashSet<Vec<String>> {
        let mut opened = HashSet::new();
        if let Some(item) = &self.issue {
            for comment in item.root_comments() {
                append_opened(&mut opened, vec![], comment.clone());
            }
        }
        opened
    }
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
    preview: PreviewState,
    section: Option<Section>,
    help: HelpState,
    // theme: Theme,
    settings: settings::Evaluated,
}

impl TryFrom<&Context> for State {
    type Error = anyhow::Error;

    fn try_from(context: &Context) -> Result<Self, Self::Error> {
        let issues = issue::all(&context.profile, &context.repository)?;
        let search = StateValue::new(context.filter.to_string());
        let filter = IssueItemFilter::from_str(&search.read()).unwrap_or_default();

        // let theme = match context.terminal_info.luma {
        //     Some(luma) if luma <= 0.6 => Theme::default_dark(),
        //     Some(luma) if luma > 0.6 => Theme::default_light(),
        //     _ => Theme::default(),
        // };

        let settings = settings::Raw::default();

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
                    .map(|comment| vec![comment.id])
                    .unwrap_or_default(),
            );
        }

        Ok(Self {
            mode: context.mode.clone(),
            pages: PageStack::new(vec![AppPage::Browser]),
            browser: BrowserState {
                items: items.clone(),
                selected: Some(0),
                filter,
                search,
                show_search: false,
            },
            preview: PreviewState {
                show: false,
                issue: items.first().cloned(),
                selected_comments,
                comment: CommentState { cursor: (0, 0) },
            },
            section: Some(Section::Browser),
            help: HelpState {
                scroll: 0,
                cursor: (0, 0),
            },
            settings: settings::Evaluated::from(settings),
        })
    }
}

pub enum Message {
    Quit,
    Exit {
        operation: Option<IssueOperation>,
    },
    ExitFromMode,
    SelectIssue {
        selected: Option<usize>,
    },
    OpenSearch,
    UpdateSearch {
        value: String,
    },
    ApplySearch,
    CloseSearch,
    TogglePreview,
    FocusSection {
        section: Option<Section>,
    },
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
            Message::Quit => Some(Exit { value: None }),
            Message::Exit { operation } => self.browser.selected_issue().map(|issue| Exit {
                value: Some(Selection {
                    operation: operation.map(|op| op.to_string()),
                    ids: vec![issue.id],
                    args: vec![],
                }),
            }),
            Message::ExitFromMode => {
                let operation = match self.mode {
                    Mode::Operation => Some(IssueOperation::Show.to_string()),
                    Mode::Id => None,
                };

                self.browser.selected_issue().map(|issue| Exit {
                    value: Some(Selection {
                        operation,
                        ids: vec![issue.id],
                        args: vec![],
                    }),
                })
            }
            Message::SelectIssue { selected } => {
                self.browser.selected = selected;
                self.preview.issue = self.browser.selected_issue().cloned();
                self.preview.comment.reset_cursor();
                None
            }
            Message::TogglePreview => {
                self.preview.show = !self.preview.show;
                self.section = Some(Section::Browser);
                None
            }
            Message::FocusSection { section } => {
                self.section = section;
                None
            }
            Message::SelectComment { selected } => {
                if let Some(item) = &self.preview.issue {
                    self.preview
                        .selected_comments
                        .insert(item.id, selected.unwrap_or(vec![]));
                }
                self.preview.comment.reset_cursor();
                None
            }
            Message::ScrollComment { cursor } => {
                self.preview.comment.update_cursor(cursor);
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
                        self.preview.issue = self.browser.issues().first().cloned();
                    } else {
                        self.preview.issue = self.browser.issues().get(selected).cloned();
                    }
                } else {
                    self.preview.issue = None;
                }

                None
            }
            Message::ApplySearch => {
                self.browser.hide_search();
                self.browser.apply_search();
                None
            }
            Message::CloseSearch => {
                self.browser.hide_search();
                self.browser.reset_search();
                self.browser.filter_items();

                self.preview.issue = self.browser.selected_issue().cloned();
                self.preview.comment.reset_cursor();
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
            .page(AppPage::Browser, browser_page(&channel))
            .page(AppPage::Help, help_page(&channel))
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

fn browser_page(channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    let shortcuts = Shortcuts::default()
        .to_widget(tx.clone())
        .on_update(|state: &State| {
            let shortcuts = if state.browser.show_search {
                vec![("esc", "cancel"), ("enter", "apply")]
            } else {
                let mut shortcuts = match state.mode {
                    Mode::Id => vec![("enter", "select")],
                    Mode::Operation => vec![("enter", "show"), ("e", "edit")],
                };
                if state.section == Some(Section::Browser) {
                    shortcuts = [shortcuts, [("/", "search")].to_vec()].concat()
                }
                [shortcuts, [("p", "preview"), ("?", "help")].to_vec()].concat()
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
                .section(issue(channel))
                .section(comment(channel))
                .to_widget(tx.clone())
                .on_event(|_, vs, _| {
                    Some(Message::FocusSection {
                        section: vs.and_then(|vs| {
                            vs.unwrap_section_group()
                                .and_then(|sgs| sgs.focus)
                                .map(|s| s.try_into().unwrap_or_default())
                        }),
                    })
                })
                .on_update(|state: &State| {
                    SectionGroupProps::default()
                        .handle_keys(state.preview.show && !state.browser.show_search)
                        .layout(PredefinedLayout::Expandable3 {
                            left_only: !state.preview.show,
                        })
                        .focus(state.section.as_ref().map(|s| s.clone().into()))
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
                    Key::Esc | Key::Ctrl('c') => Some(Message::Quit),
                    Key::Char('p') => Some(Message::TogglePreview),
                    Key::Char('?') => Some(Message::OpenHelp),
                    Key::Char('\n') => Some(Message::ExitFromMode),
                    Key::Char('e') => Some(Message::Exit {
                        operation: Some(IssueOperation::Edit),
                    }),
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

fn browser(channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    Browser::new(tx.clone())
        .to_widget(tx.clone())
        .on_update(|state| BrowserProps::from(state).to_boxed_any().into())
}

fn issue(channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    SplitContainer::default()
        .top(issue_details(channel))
        .bottom(comment_tree(channel))
        .to_widget(tx.clone())
        .on_update(|state| {
            SplitContainerProps::default()
                .heights([Constraint::Length(5), Constraint::Min(1)])
                .border_color(state.settings.theme.border_color)
                .focus_border_color(state.settings.theme.focus_border_color)
                .split_focus(SplitContainerFocus::Bottom)
                .to_boxed_any()
                .into()
        })
}

fn issue_details(channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    IssueDetails::default()
        .to_widget(tx.clone())
        .on_update(|state: &State| {
            IssueDetailsProps::default()
                .issue(state.preview.issue.clone())
                .to_boxed_any()
                .into()
        })
}

fn comment_tree(channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    Tree::<State, Message, CommentItem, String>::default()
        .to_widget(tx.clone())
        .on_event(|_, s, _| {
            Some(Message::SelectComment {
                selected: s.and_then(|s| {
                    s.unwrap_tree()
                        .map(|tree| tree.iter().map(|id| Oid::from_str(id).unwrap()).collect())
                }),
            })
        })
        .on_update(|state| {
            let root = &state.preview.root_comments();
            let opened = &state.preview.opened_comments();
            let selected = &state.preview.selected_comment_ids();

            TreeProps::<CommentItem, String>::default()
                .items(root.to_vec())
                .selected(Some(selected))
                .opened(Some(opened.clone()))
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
                    let comment = state.preview.selected_comment();
                    let body: String = comment
                        .map(|comment| comment.body.clone())
                        .unwrap_or_default();
                    let reactions = comment
                        .map(|comment| {
                            let reactions = comment.accumulated_reactions().iter().fold(
                                String::new(),
                                |all, (r, acc)| {
                                    if *acc > 1_usize {
                                        [all, format!("{}{} ", r, acc)].concat()
                                    } else {
                                        [all, format!("{} ", r)].concat()
                                    }
                                },
                            );
                            reactions
                        })
                        .unwrap_or_default();

                    TextViewProps::default()
                        .content(body)
                        .footer(Some(reactions))
                        .cursor(state.preview.comment.cursor)
                        .show_scroll_progress(true)
                        .to_boxed_any()
                        .into()
                }),
        )
        .to_widget(tx.clone())
        .on_update(|state| {
            ContainerProps::default()
                .border_color(state.settings.theme.border_color)
                .focus_border_color(state.settings.theme.focus_border_color)
                .to_boxed_any()
                .into()
        })
}

fn help_page(channel: &Channel<Message>) -> Widget<State, Message> {
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
        .to_widget(tx.clone())
        .on_update(|state| {
            ContainerProps::default()
                .border_color(state.settings.theme.border_color)
                .focus_border_color(state.settings.theme.focus_border_color)
                .to_boxed_any()
                .into()
        });

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
            Key::Esc | Key::Ctrl('c') => Some(Message::Quit),
            Key::Char('?') => Some(Message::LeavePage),
            _ => None,
        })
        .on_update(|_| PageProps::default().handle_keys(true).to_boxed_any().into())
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
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "Tab")).gray(),
                    Span::raw(": "),
                    Span::raw("focus next section").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "Backtab")).gray(),
                    Span::raw(": "),
                    Span::raw("focus previous section").gray().dim(),
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
                    Span::raw("Edit issue").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "p")).gray(),
                    Span::raw(": "),
                    Span::raw("Toggle issue preview").gray().dim(),
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

fn append_opened(all: &mut HashSet<Vec<String>>, path: Vec<String>, comment: CommentItem) {
    all.insert([path.clone(), [comment.id.to_string()].to_vec()].concat());

    for reply in comment.replies {
        append_opened(
            all,
            [path.clone(), [comment.id.to_string()].to_vec()].concat(),
            reply,
        );
    }
}
