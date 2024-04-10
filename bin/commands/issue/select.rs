#[path = "select/ui.rs"]
mod ui;

use std::str::FromStr;

use anyhow::Result;

use radicle::issue::IssueId;
use radicle::storage::git::Repository;
use radicle::Profile;

use radicle_tui as tui;

use tui::cob::issue;
use tui::store;
use tui::store::StateValue;
use tui::task;
use tui::task::Interrupted;
use tui::ui::items::{IssueItem, IssueItemFilter};
use tui::ui::Frontend;
use tui::Exit;

use ui::ListPage;

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
    issues: Vec<IssueItem>,
    mode: Mode,
    filter: IssueItemFilter,
    search: StateValue<String>,
    ui: UIState,
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

        Ok(Self {
            issues: items,
            mode: context.mode.clone(),
            filter,
            search,
            ui: UIState::default(),
        })
    }
}

pub enum Action {
    Exit { selection: Option<Selection> },
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
                self.filter = IssueItemFilter::from_str(&self.search.read()).unwrap_or_default();

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
                self.filter = IssueItemFilter::from_str(&self.search.read()).unwrap_or_default();

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
