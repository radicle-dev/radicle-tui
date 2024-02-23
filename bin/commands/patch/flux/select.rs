#[path = "select/ui.rs"]
mod ui;

use anyhow::Result;

use radicle::patch::PatchId;
use radicle::storage::git::Repository;
use radicle::Profile;

use radicle_tui as tui;

use tui::common::cob::patch::{self, Filter};
use tui::flux::store::{State, Store};
use tui::flux::termination::{self, Interrupted};
use tui::flux::ui::cob::PatchItem;
use tui::flux::ui::Frontend;
use tui::Exit;

use ui::ListPage;

use super::super::common::Mode;

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
pub struct PatchesState {
    patches: Vec<PatchItem>,
    selected: Option<PatchItem>,
    mode: Mode,
    filter: Filter,
}

impl TryFrom<&Context> for PatchesState {
    type Error = anyhow::Error;

    fn try_from(context: &Context) -> Result<Self, Self::Error> {
        let patches = patch::all(&context.repository)?;
        let patches = patches
            .iter()
            .filter(|(_, patch)| context.filter.matches(&context.profile, patch));

        let mut items = vec![];

        // Convert into UI items
        for patch in patches {
            if let Ok(item) = PatchItem::new(&context.profile, &context.repository, patch.clone()) {
                items.push(item);
            }
        }

        // Apply sorting
        items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        let selected = items.first().cloned();

        Ok(Self {
            patches: items,
            selected,
            mode: context.mode.clone(),
            filter: context.filter.clone(),
        })
    }
}

pub enum Action {
    Exit { selection: Option<Selection> },
    Select { item: PatchItem },
}

impl State<Action, Selection> for PatchesState {
    fn tick(&self) {}

    fn handle_action(&mut self, action: Action) -> Option<Exit<Selection>> {
        match action {
            Action::Select { item } => {
                self.selected = Some(item);
                None
            }
            Action::Exit { selection } => Some(Exit { value: selection }),
        }
    }
}

impl App {
    pub fn new(context: Context) -> Self {
        Self { context }
    }

    pub async fn run(&self) -> Result<Option<Selection>> {
        let (terminator, mut interrupt_rx) = termination::create_termination();
        let (store, state_rx) = Store::<Action, PatchesState, Selection>::new();
        let (frontend, action_rx) = Frontend::<Action>::new();
        let state = PatchesState::try_from(&self.context)?;

        tokio::try_join!(
            store.main_loop(state, terminator, action_rx, interrupt_rx.resubscribe()),
            frontend.main_loop::<PatchesState, ListPage, Selection>(
                state_rx,
                interrupt_rx.resubscribe()
            ),
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
