#[path = "select/ui.rs"]
mod ui;

use anyhow::Result;

use radicle::node::notifications::NotificationId;

use radicle::storage::git::Repository;
use radicle::Profile;
use radicle_tui as tui;

use tui::common::cob::inbox::{self};
use tui::flux::store::{State, Store};
use tui::flux::termination::{self, Interrupted};
use tui::flux::ui::cob::NotificationItem;
use tui::flux::ui::Frontend;
use tui::Exit;

use ui::ListPage;

use super::super::common;

type Selection = tui::Selection<NotificationId>;

pub struct Context {
    pub profile: Profile,
    pub repository: Repository,
    pub mode: common::Mode,
    pub filter: inbox::Filter,
    pub sort_by: inbox::SortBy,
}

pub struct App {
    context: Context,
}

#[derive(Clone, Debug)]
pub struct InboxState {
    notifications: Vec<NotificationItem>,
    selected: Option<NotificationItem>,
    mode: common::Mode,
}

impl TryFrom<&Context> for InboxState {
    type Error = anyhow::Error;

    fn try_from(context: &Context) -> Result<Self, Self::Error> {
        let notifications = inbox::all(&context.repository, &context.profile)?;
        let mut items = vec![];

        // Convert into UI items
        for notif in &notifications {
            if let Ok(notif) =
                NotificationItem::try_from((&context.profile, &context.repository, notif))
            {
                items.push(notif);
            }
        }

        // Apply sorting
        match context.sort_by.field {
            "timestamp" => items.sort_by(|a, b| a.timestamp.cmp(&b.timestamp)),
            "id" => items.sort_by(|a, b| a.id.cmp(&b.id)),
            _ => {}
        }
        if context.sort_by.reverse {
            items.reverse();
        }
        let selected = items.first().cloned();

        Ok(Self {
            notifications: items,
            selected,
            mode: context.mode.clone(),
        })
    }
}

pub enum Action {
    Exit { selection: Option<Selection> },
    Select { item: NotificationItem },
}

impl State<Action, Selection> for InboxState {
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
        let (store, state_rx) = Store::<Action, InboxState, Selection>::new();
        let (frontend, action_rx) = Frontend::<Action>::new();
        let state = InboxState::try_from(&self.context)?;

        tokio::try_join!(
            store.main_loop(state, terminator, action_rx, interrupt_rx.resubscribe()),
            frontend
                .main_loop::<InboxState, ListPage, Selection>(state_rx, interrupt_rx.resubscribe()),
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
