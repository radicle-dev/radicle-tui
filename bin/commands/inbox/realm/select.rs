#[path = "select/event.rs"]
mod event;
#[path = "select/page.rs"]
mod page;
#[path = "select/ui.rs"]
mod ui;

use std::fmt::Display;
use std::hash::Hash;

use anyhow::Result;

use radicle::node::notifications::NotificationId;
use serde::Serialize;

use tuirealm::application::PollStrategy;
use tuirealm::{Application, Frame, NoUserEvent, Sub, SubClause};

use radicle_tui as tui;

use tui::common::cob::inbox::{Filter, SortBy};
use tui::common::context::Context;
use tui::realm::ui::subscription;
use tui::realm::ui::theme::Theme;
use tui::realm::{PageStack, Tui};
use tui::Exit;

use page::ListView;

use super::super::common::Mode;

type Selection = tui::Selection<NotificationId>;

/// The selected issue operation returned by the operation
/// selection widget.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum InboxOperation {
    Show,
    Clear,
}

impl Display for InboxOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InboxOperation::Show => {
                write!(f, "show")
            }
            InboxOperation::Clear => {
                write!(f, "clear")
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum ListCid {
    NotificationBrowser,
    Context,
    Shortcuts,
}

/// All component ids known to this application.
#[derive(Debug, Default, Eq, PartialEq, Clone, Hash)]
pub enum Cid {
    List(ListCid),
    #[default]
    GlobalListener,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub enum Message {
    #[default]
    Tick,
    Quit(Option<Selection>),
    Batch(Vec<Message>),
}

pub struct App {
    context: Context,
    pages: PageStack<Cid, Message>,
    theme: Theme,
    mode: Mode,
    filter: Filter,
    sort_by: SortBy,
    quit: bool,
    output: Option<Selection>,
}

/// Creates a new application using a tui-realm-application, mounts all
/// components and sets focus to a default one.
#[allow(dead_code)]
impl App {
    pub fn new(context: Context, mode: Mode, filter: Filter, sort_by: SortBy) -> Self {
        Self {
            context,
            pages: PageStack::default(),
            theme: Theme::default(),
            mode,
            filter,
            sort_by,
            quit: false,
            output: None,
        }
    }

    fn view_list(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        theme: &Theme,
    ) -> Result<()> {
        let home = Box::new(ListView::new(
            self.mode.clone(),
            self.filter.clone(),
            self.sort_by,
        ));
        self.pages.push(home, app, &self.context, theme)?;

        Ok(())
    }

    fn process(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        message: Message,
    ) -> Result<Option<Message>> {
        let theme = Theme::default();
        match message {
            Message::Batch(messages) => {
                let mut results = vec![];
                for message in messages {
                    if let Some(result) = self.process(app, message)? {
                        results.push(result);
                    }
                }
                match results.len() {
                    0 => Ok(None),
                    1 => Ok(Some(results[0].to_owned())),
                    _ => Ok(Some(Message::Batch(results))),
                }
            }
            Message::Quit(output) => {
                self.quit = true;
                self.output = output;
                Ok(None)
            }
            _ => self
                .pages
                .peek_mut()?
                .update(app, &self.context, &theme, message),
        }
    }
}

impl Tui<Cid, Message, Selection> for App {
    fn init(&mut self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        self.view_list(app, &self.theme.clone())?;

        // Add global key listener and subscribe to key events
        let global = tui::realm::ui::global_listener().to_boxed();
        app.mount(
            Cid::GlobalListener,
            global,
            vec![Sub::new(
                subscription::quit_clause(tuirealm::event::Key::Char('q')),
                SubClause::Always,
            )],
        )?;

        Ok(())
    }

    fn view(&mut self, app: &mut Application<Cid, Message, NoUserEvent>, frame: &mut Frame) {
        if let Ok(page) = self.pages.peek_mut() {
            page.view(app, frame);
        }
    }

    fn update(&mut self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<bool> {
        match app.tick(PollStrategy::Once) {
            Ok(messages) if !messages.is_empty() => {
                for message in messages {
                    let mut msg = Some(message);
                    while msg.is_some() {
                        msg = self.process(app, msg.unwrap())?;
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn exit(&self) -> Option<Exit<Selection>> {
        if self.quit {
            return Some(Exit {
                value: self.output.clone(),
            });
        }
        None
    }
}
