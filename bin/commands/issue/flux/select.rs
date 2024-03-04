#[path = "select/ui.rs"]
mod ui;

use anyhow::Result;

use radicle::issue::IssueId;
use radicle::storage::git::Repository;
use radicle::Profile;

use radicle_tui as tui;

use tui::common::cob::issue::{self, Filter};
use tui::flux::store::{State, Store};
use tui::flux::termination::{self, Interrupted};
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
}

impl Default for UIState {
    fn default() -> Self {
        Self { page_size: 1 }
    }
}

#[derive(Clone, Debug)]
pub struct IssuesState {
    issues: Vec<IssueItem>,
    selected: Option<IssueItem>,
    mode: Mode,
    filter: Filter,
    ui: UIState,
}

impl TryFrom<&Context> for IssuesState {
    type Error = anyhow::Error;

    fn try_from(context: &Context) -> Result<Self, Self::Error> {
        let issues = issue::all(&context.profile, &context.repository)?;
        let issues = issues
            .iter()
            .filter(|(_, issue)| context.filter.matches(&context.profile, issue));

        let mut items = vec![];

        // Convert into UI items
        for issue in issues {
            if let Ok(item) = IssueItem::new(&context.profile, issue.clone()) {
                items.push(item);
            }
        }

        // Apply sorting
        items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        let selected = items.first().cloned();

        Ok(Self {
            issues: items,
            selected,
            mode: context.mode.clone(),
            filter: context.filter.clone(),
            ui: UIState::default(),
        })
    }
}

pub enum Action {
    Exit { selection: Option<Selection> },
    Select { item: IssueItem },
    PageSize(usize),
}

impl State<Action, Selection> for IssuesState {
    fn tick(&self) {}

    fn handle_action(&mut self, action: Action) -> Option<Exit<Selection>> {
        match action {
            Action::Select { item } => {
                self.selected = Some(item);
                None
            }
            Action::PageSize(size) => {
                self.ui.page_size = size;
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
        let (store, state_rx) = Store::<Action, IssuesState, Selection>::new();
        let (frontend, action_rx) = Frontend::<Action>::new();
        let state = IssuesState::try_from(&self.context)?;

        tokio::try_join!(
            store.main_loop(state, terminator, action_rx, interrupt_rx.resubscribe()),
            frontend.main_loop::<IssuesState, ListPage, Selection>(
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
