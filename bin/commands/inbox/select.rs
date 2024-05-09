#[path = "select/ui.rs"]
mod ui;

use std::str::FromStr;

use anyhow::Result;

use radicle::identity::Project;
use radicle::node::notifications::NotificationId;
use radicle::storage::ReadRepository;
use radicle::storage::ReadStorage;

use radicle::storage::git::Repository;
use radicle::Profile;
use radicle_tui as tui;

use tui::cob::inbox::{self};
use tui::store;
use tui::store::StateValue;
use tui::task::{self, Interrupted};
use tui::ui::items::{Filter, NotificationItem, NotificationItemFilter};
use tui::ui::widget::{Properties, Widget, Window, WindowProps};
use tui::ui::Frontend;
use tui::Exit;

use tui::PageStack;

use self::ui::BrowserPage;
use self::ui::HelpPage;

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
pub enum Page {
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
    pages: PageStack<Page>,
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
            pages: PageStack::new(vec![Page::Browse]),
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

pub enum Action {
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
    type Action = Action;

    fn tick(&self) {}

    fn handle_action(&mut self, action: Action) -> Option<Exit<Selection>> {
        match action {
            Action::Exit { selection } => Some(Exit { value: selection }),
            Action::Select { selected } => {
                self.browser.selected = selected;
                None
            }
            Action::BrowserPageSize(size) => {
                self.browser.page_size = size;
                None
            }
            Action::HelpPageSize(size) => {
                self.help.page_size = size;
                None
            }
            Action::OpenSearch => {
                self.browser.show_search = true;
                None
            }
            Action::UpdateSearch { value } => {
                self.browser.search.write(value);
                self.browser.filter = NotificationItemFilter::from_str(&self.browser.search.read())
                    .unwrap_or_default();

                None
            }
            Action::ApplySearch => {
                self.browser.search.apply();
                self.browser.show_search = false;
                None
            }
            Action::CloseSearch => {
                self.browser.search.reset();
                self.browser.show_search = false;
                self.browser.filter = NotificationItemFilter::from_str(&self.browser.search.read())
                    .unwrap_or_default();

                None
            }
            Action::OpenHelp => {
                self.pages.push(Page::Help);
                None
            }
            Action::LeavePage => {
                self.pages.pop();
                None
            }
            Action::ScrollHelp { progress } => {
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
        let (terminator, mut interrupt_rx) = task::create_termination();
        let (store, state_rx) = store::Store::<Action, State, Selection>::new();
        let (frontend, action_tx, action_rx) = Frontend::new();
        let state = State::try_from(&self.context)?;

        let window: Window<State, Action, Page> = Window::new(&state, action_tx.clone())
            .page(
                Page::Browse,
                BrowserPage::new(&state, action_tx.clone()).to_boxed(),
            )
            .page(
                Page::Help,
                HelpPage::new(&state, action_tx.clone()).to_boxed(),
            )
            .on_update(|state| {
                WindowProps::default()
                    .current_page(state.pages.peek().unwrap_or(&Page::Browse).clone())
                    .to_boxed()
            });

        tokio::try_join!(
            store.main_loop(state, terminator, action_rx, interrupt_rx.resubscribe()),
            frontend.main_loop(Some(window), state_rx, interrupt_rx.resubscribe()),
        )?;

        if let Ok(reason) = interrupt_rx.recv().await {
            match reason {
                Interrupted::User { payload } => Ok(payload),
                Interrupted::OsSignal => anyhow::bail!("exited because of an os sig int"),
            }
        } else {
            anyhow::bail!("exited because of an unexpected error");
        }
    }
}
