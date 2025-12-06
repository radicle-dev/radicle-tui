#[path = "list/ui.rs"]
mod ui;

use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;

use ratatui::Viewport;

use radicle::identity::Project;
use radicle::node::notifications::NotificationId;
use radicle::prelude::RepoId;
use radicle::storage::ReadStorage;
use radicle::Profile;

use radicle_tui as tui;

use tui::store;
use tui::task::{Process, Task};
use tui::ui::rm::widget::text::TextViewState;
use tui::ui::rm::widget::window::{Window, WindowProps};
use tui::ui::rm::widget::ToWidget;
use tui::ui::BufferedValue;
use tui::{BoxedAny, Channel, Exit, PageStack};

use crate::ui::items::filter::Filter;
use crate::ui::items::notification::filter::NotificationFilter;
use crate::ui::items::notification::filter::SortBy;
use crate::ui::items::notification::Notification;

use super::common::{Mode, RepositoryMode};

type Selection = tui::Selection<NotificationId>;

#[derive(Clone, Debug)]
pub struct Context {
    pub profile: Profile,
    pub project: Project,
    pub rid: RepoId,
    pub mode: Mode,
    pub filter: NotificationFilter,
    pub sort_by: SortBy,
}

#[derive(Default)]
pub struct App {}

impl App {
    pub async fn run(&self, context: Context) -> anyhow::Result<Option<Selection>> {
        let channel = Channel::default();
        let state = State::new(context.clone())?;
        let tx = channel.tx.clone();

        let window = Window::default()
            .page(PageState::Browse, ui::browser_page(&state, &channel))
            .page(PageState::Help, ui::help_page(&state, &channel))
            .to_widget(tx.clone())
            .on_init(|| Some(Message::Reload))
            .on_update(|state: &State| {
                WindowProps::default()
                    .current_page(state.pages.peek().unwrap_or(&PageState::Browse).clone())
                    .to_boxed_any()
                    .into()
            });

        tui::rm(
            state,
            window,
            Viewport::Inline(20),
            channel,
            vec![Loader::new(context)],
        )
        .await
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    Exit { selection: Option<Selection> },
    Select { selected: Option<usize> },
    OpenSearch,
    UpdateSearch { value: String },
    ApplySearch,
    CloseSearch,
    OpenHelp,
    LeavePage,
    ScrollHelp { state: TextViewState },
    Reload,
    NotificationsLoaded(Vec<Notification>),
}

#[derive(Clone, Debug)]
pub struct BrowserState {
    items: Arc<Mutex<Vec<Notification>>>,
    selected: Option<usize>,
    filter: NotificationFilter,
    search: BufferedValue<String>,
    show_search: bool,
    is_loading: bool,
}

impl BrowserState {
    fn items(&self) -> Vec<Notification> {
        let items = self.items.lock().unwrap();
        items
            .iter()
            .filter(|n| self.filter.matches(n))
            .cloned()
            .collect()
    }

    fn apply_filter(&mut self, filter: NotificationFilter) {
        self.filter = filter;
    }

    fn apply_notifications(&mut self, notifications: Vec<Notification>) {
        let mut items = self.items.lock().unwrap();
        *items = notifications;
    }

    fn apply_sorting(&mut self, context: &Context) {
        let mut items = self.items.lock().unwrap();
        // Apply sorting
        match context.sort_by.field {
            "timestamp" => items.sort_by(|a, b| a.timestamp.cmp(&b.timestamp)),
            "id" => items.sort_by(|a, b| a.id.cmp(&b.id)),
            _ => {}
        }
        if context.sort_by.reverse {
            items.reverse();
        }

        // Set project name
        let mode = match context.mode.repository() {
            RepositoryMode::ByRepo((rid, _)) => {
                let name = context.project.name().to_string();
                context
                    .mode
                    .clone()
                    .with_repository(RepositoryMode::ByRepo((*rid, Some(name))))
            }
            _ => context.mode.clone(),
        };

        // Sort by project if all notifications are shown
        if let RepositoryMode::All = mode.repository() {
            items.sort_by(|a, b| a.project.cmp(&b.project));
        }
    }
}

#[derive(Clone, Debug)]
pub struct HelpState {
    text: TextViewState,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum PageState {
    Browse,
    Help,
}

#[derive(Clone, Debug)]
pub struct State {
    pages: PageStack<PageState>,
    browser: BrowserState,
    help: HelpState,
    context: Arc<Mutex<Context>>,
}

impl State {
    fn new(context: Context) -> Result<Self, anyhow::Error> {
        let search = BufferedValue::new(context.filter.to_string());

        Ok(Self {
            pages: PageStack::new(vec![PageState::Browse]),
            browser: BrowserState {
                items: Arc::new(Mutex::new(vec![])),
                selected: Some(0),
                filter: context.filter.clone(),
                search,
                show_search: false,
                is_loading: false,
            },
            help: HelpState {
                text: TextViewState::default().content(ui::help_text()),
            },
            context: Arc::new(Mutex::new(context)),
        })
    }
}

impl store::Update<Message> for State {
    type Return = Selection;

    fn update(&mut self, message: Message) -> Option<Exit<Selection>> {
        match message {
            Message::Exit { selection } => Some(Exit { value: selection }),
            Message::Select { selected } => {
                self.browser.selected = selected;
                None
            }
            Message::OpenSearch => {
                self.browser.show_search = true;
                None
            }
            Message::UpdateSearch { value } => {
                self.browser.search.write(value);
                self.browser.apply_filter(
                    NotificationFilter::from_str(&self.browser.search.read())
                        .unwrap_or(NotificationFilter::Invalid),
                );

                let items = self.browser.items.lock().unwrap();
                if let Some(selected) = self.browser.selected {
                    if selected > items.len() {
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
                self.browser.apply_filter(
                    NotificationFilter::from_str(&self.browser.search.read())
                        .unwrap_or(NotificationFilter::Invalid),
                );

                None
            }
            Message::OpenHelp => {
                self.pages.push(PageState::Help);
                None
            }
            Message::LeavePage => {
                self.pages.pop();
                None
            }
            Message::ScrollHelp { state } => {
                self.help.text = state;
                None
            }
            Message::Reload => {
                self.browser.is_loading = true;
                None
            }
            Message::NotificationsLoaded(notifications) => {
                let context = self.context.lock().unwrap();
                self.browser.apply_notifications(notifications);
                self.browser.apply_filter(self.browser.filter.clone());
                self.browser.apply_sorting(&context);
                self.browser.is_loading = false;
                None
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Loader {
    context: Context,
}

impl Loader {
    fn new(context: Context) -> Self {
        Self { context }
    }
}

#[derive(Debug)]
pub struct NotificationLoader {
    context: Context,
}

impl NotificationLoader {
    fn new(context: Context) -> Self {
        NotificationLoader { context }
    }
}

impl Task for NotificationLoader {
    type Return = Message;

    fn run(&self) -> anyhow::Result<Vec<Self::Return>> {
        let notifications = match self.context.mode.repository() {
            RepositoryMode::All => {
                let notifs = self.context.profile.notifications_mut()?;
                let all = notifs.all()?;

                all.filter_map(|notif| notif.ok())
                    .map(|notif| {
                        let repo = self.context.profile.storage.repository(notif.repo)?;
                        Notification::new(
                            &self.context.profile,
                            &self.context.project,
                            &repo,
                            &notif,
                        )
                    })
                    .filter_map(|notif| notif.ok())
                    .flatten()
                    .collect::<Vec<_>>()
            }
            RepositoryMode::Contextual => {
                let repo = self.context.profile.storage.repository(self.context.rid)?;
                let notifs = self.context.profile.notifications_mut()?;
                let by_repo = notifs.by_repo(&repo.id, "timestamp")?;

                by_repo
                    .filter_map(|notif| notif.ok())
                    .map(|notif| {
                        let repo = self.context.profile.storage.repository(notif.repo)?;
                        Notification::new(
                            &self.context.profile,
                            &self.context.project,
                            &repo,
                            &notif,
                        )
                    })
                    .filter_map(|notif| notif.ok())
                    .flatten()
                    .collect::<Vec<_>>()
            }
            RepositoryMode::ByRepo((rid, _)) => {
                let repo = self.context.profile.storage.repository(*rid)?;
                let notifs = self.context.profile.notifications_mut()?;
                let by_repo = notifs.by_repo(&repo.id, "timestamp")?;

                by_repo
                    .filter_map(|notif| notif.ok())
                    .map(|notif| {
                        let repo = self.context.profile.storage.repository(notif.repo)?;
                        Notification::new(
                            &self.context.profile,
                            &self.context.project,
                            &repo,
                            &notif,
                        )
                    })
                    .filter_map(|notif| notif.ok())
                    .flatten()
                    .collect::<Vec<_>>()
            }
        };

        Ok(vec![Message::NotificationsLoaded(notifications)])
    }
}

impl Process<Message> for Loader {
    async fn process(&mut self, message: Message) -> anyhow::Result<Vec<Message>> {
        match message {
            Message::Reload => {
                let loader = NotificationLoader::new(self.context.clone());
                let messages = tokio::spawn(async move { loader.run() }).await.unwrap()?;
                Ok(messages)
            }
            _ => Ok(vec![]),
        }
    }
}
