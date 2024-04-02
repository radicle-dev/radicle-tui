#[path = "select/ui.rs"]
mod ui;

use anyhow::Result;

use radicle::identity::Project;
use radicle::node::notifications::NotificationId;
use radicle::storage::ReadRepository;
use radicle::storage::ReadStorage;

use radicle::storage::git::Repository;
use radicle::Profile;
use radicle_tui as tui;

use tui::common::cob::inbox::{self};
use tui::flux::store;
use tui::flux::store::StateValue;
use tui::flux::task::{self, Interrupted};
use tui::flux::ui::items::NotificationItem;
use tui::flux::ui::Frontend;
use tui::Exit;

use ui::ListPage;

use super::super::common::{Mode, RepositoryMode};

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

#[derive(Clone, Debug)]
pub struct UIState {
    page_size: usize,
    show_search: bool,
    show_help: bool,
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            page_size: 1,
            show_search: false,
            show_help: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct State {
    notifications: Vec<NotificationItem>,
    selected: Option<NotificationItem>,
    mode: Mode,
    project: Project,
    search: StateValue<String>,
    ui: UIState,
}

impl TryFrom<&Context> for State {
    type Error = anyhow::Error;

    fn try_from(context: &Context) -> Result<Self, Self::Error> {
        let doc = context.repository.identity_doc()?;
        let project = doc.project()?;

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

        let selected = notifications.first().cloned();

        Ok(Self {
            notifications,
            selected,
            mode: mode.clone(),
            project,
            search: StateValue::new(String::new()),
            ui: UIState::default(),
        })
    }
}

pub enum Action {
    Exit { selection: Option<Selection> },
    Select { item: NotificationItem },
    PageSize(usize),
    OpenSearch,
    UpdateSearch { value: String },
    ApplySearch,
    CloseSearch,
    OpenHelp,
    CloseHelp,
}

impl store::State<Action, Selection> for State {
    fn tick(&self) {}

    fn handle_action(&mut self, action: Action) -> Option<Exit<Selection>> {
        match action {
            Action::Exit { selection } => Some(Exit { value: selection }),
            Action::Select { item } => {
                self.selected = Some(item);
                None
            }
            Action::PageSize(size) => {
                self.ui.page_size = size;
                None
            }
            Action::OpenSearch => {
                self.ui.show_search = true;
                None
            }
            Action::UpdateSearch { value } => {
                self.search.write(value);
                None
            }
            Action::ApplySearch => {
                self.search.apply();
                self.ui.show_search = false;
                None
            }
            Action::CloseSearch => {
                self.search.reset();
                self.ui.show_search = false;
                None
            }
            Action::OpenHelp => {
                self.ui.show_help = true;
                None
            }
            Action::CloseHelp => {
                self.ui.show_help = false;
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
        let (frontend, action_rx) = Frontend::<Action>::new();
        let state = State::try_from(&self.context)?;

        tokio::try_join!(
            store.main_loop(state, terminator, action_rx, interrupt_rx.resubscribe()),
            frontend.main_loop::<State, ListPage, Selection>(state_rx, interrupt_rx.resubscribe()),
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
