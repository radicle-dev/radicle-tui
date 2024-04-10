#[path = "select/ui.rs"]
mod ui;

use std::str::FromStr;

use anyhow::Result;

use radicle::patch::PatchId;
use radicle::storage::git::Repository;
use radicle::Profile;

use radicle_tui as tui;

use tui::cob::patch;
use tui::cob::patch::Filter;
use tui::store;
use tui::task;
use tui::task::Interrupted;
use tui::ui::items::{PatchItem, PatchItemFilter};
use tui::ui::Frontend;
use tui::Exit;

use ui::ListPage;

use super::common::Mode;

type Selection = tui::Selection<PatchId>;

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
    patches: Vec<PatchItem>,
    mode: Mode,
    filter: PatchItemFilter,
    search: store::StateValue<String>,
    ui: UIState,
}

impl TryFrom<&Context> for State {
    type Error = anyhow::Error;

    fn try_from(context: &Context) -> Result<Self, Self::Error> {
        let patches = patch::all(&context.profile, &context.repository)?;
        let search = store::StateValue::new(context.filter.to_string());
        let filter = PatchItemFilter::from_str(&context.filter.to_string()).unwrap_or_default();

        // Convert into UI items
        let mut items = vec![];
        for patch in patches {
            if let Ok(item) = PatchItem::new(&context.profile, &context.repository, patch.clone()) {
                items.push(item);
            }
        }
        items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(Self {
            patches: items,
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
                self.filter = PatchItemFilter::from_str(&self.search.read()).unwrap_or_default();

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
                self.filter = PatchItemFilter::from_str(&self.search.read()).unwrap_or_default();

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
