#[path = "select/ui.rs"]
mod ui;

use std::str::FromStr;

use anyhow::Result;

use termion::event::Key;

use ratatui::layout::Constraint;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;

use radicle::identity::Project;
use radicle::node::notifications::NotificationId;
use radicle::storage::git::Repository;
use radicle::storage::ReadRepository;
use radicle::storage::ReadStorage;
use radicle::Profile;

use radicle_tui as tui;

use tui::cob::inbox;
use tui::store;
use tui::store::StateValue;
use tui::ui::items::{Filter, NotificationItem, NotificationItemFilter};
use tui::ui::span;
use tui::ui::widget::container::{Column, Container, Footer, FooterProps, Header, HeaderProps};
use tui::ui::widget::text::{TextArea, TextAreaProps};
use tui::ui::widget::window::{Page, PageProps, Shortcuts, ShortcutsProps, Window, WindowProps};
use tui::ui::widget::{ToWidget, Widget};
use tui::{BoxedAny, Channel, Exit, PageStack};

use self::ui::Browser;
use self::ui::BrowserProps;

use super::common::SelectionMode;
use super::common::{Mode, RepositoryMode};

type Selection = tui::Selection<NotificationId>;

pub struct Context {
    pub profile: Profile,
    pub repository: Repository,
    pub mode: Mode,
    pub filter: inbox::Filter,
    pub sort_by: inbox::SortBy,
}

pub struct App {
    context: Context,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum AppPage {
    Browse,
    Help,
}

#[derive(Clone, Debug)]
pub struct BrowserState {
    items: Vec<NotificationItem>,
    selected: Option<usize>,
    filter: NotificationItemFilter,
    search: store::StateValue<String>,
    page_size: usize,
    show_search: bool,
}

impl BrowserState {
    pub fn notifications(&self) -> Vec<NotificationItem> {
        self.items
            .iter()
            .filter(|patch| self.filter.matches(patch))
            .cloned()
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct HelpState {
    progress: usize,
    page_size: usize,
}

#[derive(Clone, Debug)]
pub struct State {
    mode: Mode,
    project: Project,
    pages: PageStack<AppPage>,
    browser: BrowserState,
    help: HelpState,
}

impl TryFrom<&Context> for State {
    type Error = anyhow::Error;

    fn try_from(context: &Context) -> Result<Self, Self::Error> {
        let doc = context.repository.identity_doc()?;
        let project = doc.project()?;

        let search = StateValue::new(String::new());
        let filter = NotificationItemFilter::from_str(&search.read()).unwrap_or_default();

        let mut notifications = match &context.mode.repository() {
            RepositoryMode::All => {
                let mut repos = context.profile.storage.repositories()?;
                repos.sort_by_key(|r| r.rid);

                let mut notifs = vec![];
                for repo in repos {
                    let repo = context.profile.storage.repository(repo.rid)?;

                    let items = inbox::all(&repo, &context.profile)?
                        .iter()
                        .map(|notif| NotificationItem::new(&context.profile, &repo, notif))
                        .filter_map(|item| item.ok())
                        .flatten()
                        .collect::<Vec<_>>();

                    notifs.extend(items);
                }

                notifs
            }
            RepositoryMode::Contextual => {
                let notifs = inbox::all(&context.repository, &context.profile)?;

                notifs
                    .iter()
                    .map(|notif| {
                        NotificationItem::new(&context.profile, &context.repository, notif)
                    })
                    .filter_map(|item| item.ok())
                    .flatten()
                    .collect::<Vec<_>>()
            }
            RepositoryMode::ByRepo((rid, _)) => {
                let repo = context.profile.storage.repository(*rid)?;
                let notifs = inbox::all(&repo, &context.profile)?;

                notifs
                    .iter()
                    .map(|notif| NotificationItem::new(&context.profile, &repo, notif))
                    .filter_map(|item| item.ok())
                    .flatten()
                    .collect::<Vec<_>>()
            }
        };

        // Set project name
        let mode = match &context.mode.repository() {
            RepositoryMode::ByRepo((rid, _)) => {
                let project = context
                    .profile
                    .storage
                    .repository(*rid)?
                    .identity_doc()?
                    .project()?;
                let name = project.name().to_string();

                context
                    .mode
                    .clone()
                    .with_repository(RepositoryMode::ByRepo((*rid, Some(name))))
            }
            _ => context.mode.clone(),
        };

        // Apply sorting
        match context.sort_by.field {
            "timestamp" => notifications.sort_by(|a, b| a.timestamp.cmp(&b.timestamp)),
            "id" => notifications.sort_by(|a, b| a.id.cmp(&b.id)),
            _ => {}
        }
        if context.sort_by.reverse {
            notifications.reverse();
        }

        // Sort by project if all notifications are shown
        if let RepositoryMode::All = mode.repository() {
            notifications.sort_by(|a, b| a.project.cmp(&b.project));
        }

        Ok(Self {
            mode: context.mode.clone(),
            project,
            pages: PageStack::new(vec![AppPage::Browse]),
            browser: BrowserState {
                items: notifications,
                selected: Some(0),
                filter,
                search,
                show_search: false,
                page_size: 1,
            },
            help: HelpState {
                progress: 0,
                page_size: 1,
            },
        })
    }
}

pub enum Message {
    Exit { selection: Option<Selection> },
    Select { selected: Option<usize> },
    BrowserPageSize(usize),
    HelpPageSize(usize),
    OpenSearch,
    UpdateSearch { value: String },
    ApplySearch,
    CloseSearch,
    OpenHelp,
    LeavePage,
    ScrollHelp { progress: usize },
}

impl store::State<Selection> for State {
    type Message = Message;

    fn update(&mut self, message: Message) -> Option<Exit<Selection>> {
        match message {
            Message::Exit { selection } => Some(Exit { value: selection }),
            Message::Select { selected } => {
                self.browser.selected = selected;
                None
            }
            Message::BrowserPageSize(size) => {
                self.browser.page_size = size;
                None
            }
            Message::HelpPageSize(size) => {
                self.help.page_size = size;
                None
            }
            Message::OpenSearch => {
                self.browser.show_search = true;
                None
            }
            Message::UpdateSearch { value } => {
                self.browser.search.write(value);
                self.browser.filter = NotificationItemFilter::from_str(&self.browser.search.read())
                    .unwrap_or_default();

                if let Some(selected) = self.browser.selected {
                    if selected > self.browser.notifications().len() {
                        self.browser.selected = Some(0);
                    }
                }

                None
            }
            Message::ApplySearch => {
                self.browser.search.apply();
                self.browser.show_search = false;
                None
            }
            Message::CloseSearch => {
                self.browser.search.reset();
                self.browser.show_search = false;
                self.browser.filter = NotificationItemFilter::from_str(&self.browser.search.read())
                    .unwrap_or_default();

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
            Message::ScrollHelp { progress } => {
                self.help.progress = progress;
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
            .page(AppPage::Browse, browser_page(&state, &channel))
            .page(AppPage::Help, help_page(&state, &channel))
            .to_widget(tx.clone())
            .on_update(|state: &State| {
                WindowProps::default()
                    .current_page(state.pages.peek().unwrap_or(&AppPage::Browse).clone())
                    .to_boxed_any()
                    .into()
            });

        tui::run(channel, state, window).await
    }
}

fn browser_page(_state: &State, channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    let content = Browser::new(tx.clone())
        .to_widget(tx.clone())
        .on_update(|state| BrowserProps::from(state).to_boxed_any().into());

    let shortcuts = Shortcuts::default()
        .to_widget(tx.clone())
        .on_update(|state: &State| {
            let shortcuts = if state.browser.show_search {
                vec![("esc", "cancel"), ("enter", "apply")]
            } else {
                match state.mode.selection() {
                    SelectionMode::Id => vec![("enter", "select"), ("/", "search")],
                    SelectionMode::Operation => vec![
                        ("enter", "show"),
                        ("c", "clear"),
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
        .content(content)
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
                .page_size(state.browser.page_size)
                .handle_keys(!state.browser.show_search)
                .to_boxed_any()
                .into()
        })
        .on_render(|props, render| {
            let default = PageProps::default();
            let props = props
                .and_then(|props| props.inner_ref::<PageProps>())
                .unwrap_or(&default);
            let page_size = render.area.height.saturating_sub(6) as usize;

            if page_size != props.page_size {
                return Some(Message::BrowserPageSize(page_size));
            }
            None
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
            TextArea::default()
                .to_widget(tx.clone())
                .on_event(|_, s, _| {
                    Some(Message::ScrollHelp {
                        progress: s.and_then(|p| p.unwrap_usize()).unwrap_or_default(),
                    })
                })
                .on_update(|state: &State| {
                    TextAreaProps::default()
                        .text(&help_text())
                        .page_size(state.help.page_size)
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
                                    span::default(&format!("{}%", state.help.progress)).dim(),
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
        .on_update(|state: &State| {
            PageProps::default()
                .page_size(state.help.page_size)
                .handle_keys(true)
                .to_boxed_any()
                .into()
        })
        .on_render(|props, render| {
            let default = PageProps::default();
            let props = props
                .and_then(|props| props.inner_ref::<PageProps>())
                .unwrap_or(&default);
            let page_size = render.area.height.saturating_sub(6) as usize;

            if page_size != props.page_size {
                return Some(Message::HelpPageSize(page_size));
            }
            None
        })
}

fn help_text() -> Text<'static> {
    Text::from(
        [
            Line::from(Span::raw("Generic keybindings").cyan()),
            Line::raw(""),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "↑,k")).gray(),
                Span::raw(" "),
                Span::raw("move cursor one line up").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "↓,j")).gray(),
                Span::raw(" "),
                Span::raw("move cursor one line down").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "PageUp")).gray(),
                Span::raw(" "),
                Span::raw("move cursor one page up").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "PageDown")).gray(),
                Span::raw(" "),
                Span::raw("move cursor one page down").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "Home")).gray(),
                Span::raw(" "),
                Span::raw("move cursor to the first line").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "End")).gray(),
                Span::raw(" "),
                Span::raw("move cursor to the last line").gray().dim(),
            ]),
            Line::raw(""),
            Line::from(Span::raw("Specific keybindings").cyan()),
            Line::raw(""),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "enter")).gray(),
                Span::raw(" "),
                Span::raw("Select notification (if --mode id)").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "enter")).gray(),
                Span::raw(" "),
                Span::raw("Show notification").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "c")).gray(),
                Span::raw(" "),
                Span::raw("Clear notifications").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "/")).gray(),
                Span::raw(" "),
                Span::raw("Search").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "?")).gray(),
                Span::raw(" "),
                Span::raw("Show help").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "Esc")).gray(),
                Span::raw(" "),
                Span::raw("Quit / cancel").gray().dim(),
            ]),
            Line::raw(""),
            Line::from(Span::raw("Searching").cyan()),
            Line::raw(""),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "Pattern")).gray(),
                Span::raw(" "),
                Span::raw("is:<state> | is:patch | is:issue | <search>")
                    .gray()
                    .dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "Example")).gray(),
                Span::raw(" "),
                Span::raw("is:unseen is:patch Print").gray().dim(),
            ]),
            Line::raw(""),
            Line::raw(""),
        ]
        .to_vec(),
    )
}
