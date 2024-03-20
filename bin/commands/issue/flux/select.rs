#[path = "select/ui.rs"]
mod ui;

use anyhow::Result;

use radicle::issue::IssueId;
use radicle::storage::git::Repository;
use radicle::Profile;

use radicle_tui as tui;

use tui::common::cob::issue::{self, Filter};
use tui::flux::store::{self, StateValue};
use tui::flux::task::{self, Interrupted};
use tui::flux::ui::cob::IssueItem;
use tui::flux::ui::Frontend;
use tui::Exit;

use ui::ListPage;

use super::super::common::Mode;

type Selection = tui::Selection<IssueId>;

pub struct Context {
    pub profile: Profile,
    pub repository: Repository,
    pub mode: Mode,
    pub filter: Filter,
}

pub struct App {
    context: Context,
}

#[derive(Clone, Debug)]
pub struct UIState {
    page_size: usize,
    show_search: bool,
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            page_size: 1,
            show_search: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct State {
    issues: Vec<IssueItem>,
    selected: Option<IssueItem>,
    mode: Mode,
    search: StateValue<String>,
    ui: UIState,
}

impl TryFrom<&Context> for State {
    type Error = anyhow::Error;

    fn try_from(context: &Context) -> Result<Self, Self::Error> {
        let issues = issue::all(&context.profile, &context.repository)?;

        // Convert into UI items
        let mut items = vec![];
        for issue in issues {
            if let Ok(item) = IssueItem::new(&context.profile, issue.clone()) {
                items.push(item);
            }
        }

        let selected = items.first().cloned();

        Ok(Self {
            issues: items,
            selected,
            mode: context.mode.clone(),
            search: StateValue::new(context.filter.to_string()),
            ui: UIState::default(),
        })
    }
}

pub enum Action {
    Exit { selection: Option<Selection> },
    Select { item: IssueItem },
    PageSize(usize),
    OpenSearch,
    UpdateSearch { value: String },
    ApplySearch,
    CloseSearch,
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
