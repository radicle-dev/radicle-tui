#[path = "flux/ui.rs"]
mod ui;

use anyhow::Result;

use radicle::node::notifications::NotificationId;

use radicle_tui as tui;

use tui::cob::inbox::{self, Filter};
use tui::context::Context;
use tui::flux::store::{State, Store};
use tui::flux::termination::{self, Interrupted};
use tui::flux::ui::cob::NotificationItem;
use tui::flux::ui::Frontend;
use tui::Exit;

use crate::tui_inbox::select::flux::ui::ListPage;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum Mode {
    Id,
    #[default]
    Operation,
}

pub struct App {
    context: Context,
    _filter: Filter,
}

#[derive(Clone, Debug)]
pub struct InboxState {
    notifications: Vec<NotificationItem>,
    selected: Option<NotificationId>,
}

impl TryFrom<&Context> for InboxState {
    type Error = anyhow::Error;

    fn try_from(context: &Context) -> Result<Self, Self::Error> {
        let notifications = inbox::all(context.repository(), context.profile())?;
        let mut items = vec![];

        for notif in &notifications {
            if let Ok(notif) = NotificationItem::try_from((context.repository(), notif)) {
                items.push(notif);
            }
        }

        Ok(Self {
            notifications: items,
            selected: None,
        })
    }
}

pub enum Action {
    Exit,
    Select(NotificationId),
}

impl State<Action> for InboxState {
    type Exit = Exit<String>;

    fn tick(&self) {}

    fn handle_action(&mut self, action: Action) -> Option<Exit<String>> {
        match action {
            Action::Select(id) => {
                self.selected = Some(id);
                None
            }
            Action::Exit => Some(Exit { value: None }),
        }
    }
}

impl App {
    pub fn new(context: Context, filter: Filter) -> Self {
        Self {
            context,
            _filter: filter,
        }
    }

    pub async fn run(&self) -> Result<()> {
        let (terminator, mut interrupt_rx) = termination::create_termination();
        let (store, state_rx) = Store::<Action, InboxState>::new();
        let (frontend, action_rx) = Frontend::<Action>::new();
        let state = InboxState::try_from(&self.context)?;

        tokio::try_join!(
            store.main_loop(state, terminator, action_rx, interrupt_rx.resubscribe()),
            frontend.main_loop::<InboxState, ListPage>(state_rx, interrupt_rx.resubscribe()),
        )?;

        if let Ok(reason) = interrupt_rx.recv().await {
            match reason {
                Interrupted::UserInt => {}
                Interrupted::OsSigInt => println!("exited because of an os sig int"),
            }
        } else {
            println!("exited because of an unexpected error");
        }

        Ok(())
    }
}
