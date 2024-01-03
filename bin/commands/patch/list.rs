#[path = "list/event.rs"]
mod event;
#[path = "list/page.rs"]
mod page;
#[path = "list/ui.rs"]
mod ui;

use std::hash::Hash;

use anyhow::Result;

use radicle::cob::patch::PatchId;

use tuirealm::application::PollStrategy;
use tuirealm::{Application, Frame, NoUserEvent, Sub, SubClause};

use radicle_tui as tui;

use tui::context::Context;
use tui::ui::subscription;
use tui::ui::theme::{self, Theme};
use tui::{Exit, PageStack, Tui};

use page::ListView;

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum ListCid {
    Header,
    PatchBrowser,
    Context,
    Shortcuts,
}

/// All component ids known to this application.
#[derive(Debug, Default, Eq, PartialEq, Clone, Hash)]
pub enum Cid {
    List(ListCid),
    #[default]
    GlobalListener,
    Popup,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PopupMessage {
    Info(String),
    Warning(String),
    Error(String),
    Hide,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub enum Message {
    Popup(PopupMessage),
    #[default]
    Tick,
    Quit(Option<PatchId>),
    Batch(Vec<Message>),
}

#[allow(dead_code)]
pub struct App {
    context: Context,
    pages: PageStack<Cid, Message>,
    theme: Theme,
    quit: bool,
    patch_id: Option<PatchId>,
}

/// Creates a new application using a tui-realm-application, mounts all
/// components and sets focus to a default one.
impl App {
    pub fn new(context: Context) -> Self {
        Self {
            context,
            pages: PageStack::default(),
            theme: theme::default_dark(),
            quit: false,
            patch_id: None,
        }
    }

    fn view_list(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        theme: &Theme,
    ) -> Result<()> {
        let home = Box::new(ListView::new(theme.clone()));
        self.pages.push(home, app, &self.context, theme)?;

        Ok(())
    }

    fn process(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        message: Message,
    ) -> Result<Option<Message>> {
        let theme = theme::default_dark();
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
            Message::Popup(PopupMessage::Info(info)) => {
                self.show_info_popup(app, &theme, &info)?;
                Ok(None)
            }
            Message::Popup(PopupMessage::Warning(warning)) => {
                self.show_warning_popup(app, &theme, &warning)?;
                Ok(None)
            }
            Message::Popup(PopupMessage::Error(error)) => {
                self.show_error_popup(app, &theme, &error)?;
                Ok(None)
            }
            Message::Popup(PopupMessage::Hide) => {
                self.hide_popup(app)?;
                Ok(None)
            }
            Message::Quit(id) => {
                self.quit = true;
                self.patch_id = id;
                Ok(None)
            }
            _ => self
                .pages
                .peek_mut()?
                .update(app, &self.context, &theme, message),
        }
    }

    fn show_info_popup(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        theme: &Theme,
        message: &str,
    ) -> Result<()> {
        let popup = tui::ui::info(theme, message);
        app.remount(Cid::Popup, popup.to_boxed(), vec![])?;
        app.active(&Cid::Popup)?;

        Ok(())
    }

    fn show_warning_popup(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        theme: &Theme,
        message: &str,
    ) -> Result<()> {
        let popup = tui::ui::warning(theme, message);
        app.remount(Cid::Popup, popup.to_boxed(), vec![])?;
        app.active(&Cid::Popup)?;

        Ok(())
    }

    fn show_error_popup(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        theme: &Theme,
        message: &str,
    ) -> Result<()> {
        let popup = tui::ui::error(theme, message);
        app.remount(Cid::Popup, popup.to_boxed(), vec![])?;
        app.active(&Cid::Popup)?;

        Ok(())
    }

    fn hide_popup(&mut self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.blur()?;
        app.umount(&Cid::Popup)?;

        Ok(())
    }
}

impl Tui<Cid, Message> for App {
    fn init(&mut self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        self.view_list(app, &self.theme.clone())?;

        // Add global key listener and subscribe to key events
        let global = tui::ui::global_listener().to_boxed();
        app.mount(
            Cid::GlobalListener,
            global,
            vec![Sub::new(subscription::global_clause(), SubClause::Always)],
        )?;

        Ok(())
    }

    fn view(&mut self, app: &mut Application<Cid, Message, NoUserEvent>, frame: &mut Frame) {
        if let Ok(page) = self.pages.peek_mut() {
            page.view(app, frame);
        }

        if app.mounted(&Cid::Popup) {
            app.view(&Cid::Popup, frame, frame.size());
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

    fn exit(&self) -> Option<Exit> {
        if self.quit {
            return Some(Exit {
                value: self.patch_id.map(|id| format!("{id}")),
            });
        }
        None
    }
}
