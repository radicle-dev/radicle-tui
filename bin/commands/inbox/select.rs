#[path = "select/event.rs"]
mod event;
#[path = "select/page.rs"]
mod page;
#[path = "select/ui.rs"]
mod ui;

use std::fmt::Display;
use std::hash::Hash;

use anyhow::Result;
use serde::{Serialize, Serializer};

use tuirealm::application::PollStrategy;
use tuirealm::event::Key;
use tuirealm::{Application, Frame, NoUserEvent, Sub, SubClause};

use radicle_tui as tui;

use tui::cob::inbox::Filter;
use tui::context::Context;

use tui::ui::subscription;
use tui::ui::theme::Theme;
use tui::{Exit, PageStack, SelectionExit, Tui};

use page::ListView;

/// Wrapper around radicle's `PatchId` that serializes
/// to a human-readable string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatchId(radicle::cob::patch::PatchId);

impl From<radicle::cob::patch::PatchId> for PatchId {
    fn from(value: radicle::cob::patch::PatchId) -> Self {
        PatchId(value)
    }
}

impl Display for PatchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for PatchId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}", *self.0))
    }
}

/// The application's subject. It tells the application
/// which widgets to render and which output to produce.
///
/// Depends on CLI arguments given by the user.
#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Operation,
    Id,
}

/// The selected issue operation returned by the operation
/// selection widget.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum IssueOperation {
    Show,
    Delete,
    Edit,
    Comment,
}

impl Display for IssueOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IssueOperation::Show => {
                write!(f, "show")
            }
            IssueOperation::Delete => {
                write!(f, "delete")
            }
            IssueOperation::Edit => {
                write!(f, "edit")
            }
            IssueOperation::Comment => {
                write!(f, "comment")
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
    Quit(Option<SelectionExit>),
    Batch(Vec<Message>),
}

pub struct App {
    context: Context,
    pages: PageStack<Cid, Message>,
    theme: Theme,
    quit: bool,
    filter: Filter,
    output: Option<SelectionExit>,
}

/// Creates a new application using a tui-realm-application, mounts all
/// components and sets focus to a default one.
impl App {
    pub fn new(context: Context, filter: Filter) -> Self {
        Self {
            context,
            pages: PageStack::default(),
            theme: Theme::default(),
            quit: false,
            filter,
            output: None,
        }
    }

    fn view_list(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        theme: &Theme,
    ) -> Result<()> {
        let home = Box::new(ListView::new(self.filter.clone()));
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

impl Tui<Cid, Message, SelectionExit> for App {
    fn init(&mut self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        self.view_list(app, &self.theme.clone())?;

        // Add global key listener and subscribe to key events
        let global = tui::ui::global_listener().to_boxed();
        app.mount(
            Cid::GlobalListener,
            global,
            vec![Sub::new(
                subscription::quit_clause(Key::Char('q')),
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

    fn exit(&self) -> Option<Exit<SelectionExit>> {
        if self.quit {
            return Some(Exit {
                value: self.output.clone(),
            });
        }
        None
    }
}
