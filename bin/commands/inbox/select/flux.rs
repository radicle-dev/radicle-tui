#[path = "flux/ui.rs"]
mod ui;

use std::default;

use anyhow::Result;

use radicle::node::notifications::Notification;
use radicle_tui as tui;
use tui::cob::inbox::Filter;
use tui::context::Context;
use tui::flux::store::{State, Store};
use tui::flux::termination::{self, Interrupted};
use tui::flux::ui::Frontend;

use crate::tui_inbox::select::flux::ui::ListPage;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum Mode {
    Id,
    #[default]
    Operation,
}

pub struct App {
    context: Context,
    filter: Filter,
}

#[derive(Clone)]
pub struct InboxState {
    notifications: Vec<String>,
}

impl InboxState {
    pub fn notifications(&self) -> &Vec<String> {
        &self.notifications
    }
}

pub enum Action {
    Exit,
}

impl State<Action> for InboxState {
    fn tick(&self) {}

    fn handle_action(self, action: Action) -> anyhow::Result<bool> {
        match action {
            Action::Exit => Ok(true),
        }
    }
}

impl App {
    pub fn new(context: Context, filter: Filter) -> Self {
        Self { context, filter }
    }

    pub async fn run(&self) -> Result<()> {
        let (terminator, mut interrupt_rx) = termination::create_termination();
        let (store, state_rx) = Store::<Action, InboxState>::new();
        let (frontend, action_rx) = Frontend::<Action>::new();
        let state = InboxState {
            notifications: vec![],
        };

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
