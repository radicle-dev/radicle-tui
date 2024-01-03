#[path = "suite/event.rs"]
mod event;
#[path = "suite/page.rs"]
mod page;
#[path = "suite/ui.rs"]
mod ui;

use anyhow::Result;

use radicle::cob::issue::IssueId;

use tuirealm::application::PollStrategy;
use tuirealm::event::Key;
use tuirealm::{Application, Frame, NoUserEvent, Sub, SubClause};

use radicle_tui as tui;

use tui::cob;
use tui::context::Context;
use tui::ui::subscription;
use tui::ui::theme::{self, Theme};
use tui::{Exit, PageStack, Tui};

use page::{IssuePage, ListPage};

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum ListCid {
    Header,
    IssueBrowser,
    Context,
    Shortcuts,
}

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum IssueCid {
    Header,
    List,
    Details,
    Context,
    Form,
    Shortcuts,
}

/// All component ids known to this application.
#[derive(Debug, Default, Eq, PartialEq, Clone, Hash)]
pub enum Cid {
    List(ListCid),
    Issue(IssueCid),
    #[default]
    GlobalListener,
    Popup,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IssueCobMessage {
    Create {
        title: String,
        tags: String,
        assignees: String,
        description: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IssueMessage {
    Show(Option<IssueId>),
    Changed(IssueId),
    Focus(IssueCid),
    Created(IssueId),
    Cob(IssueCobMessage),
    Reload(Option<IssueId>),
    OpenForm,
    HideForm,
    Leave(Option<IssueId>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PopupMessage {
    Info(String),
    Warning(String),
    Error(String),
    Hide,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Message {
    Issue(IssueMessage),
    NavigationChanged(u16),
    FormSubmitted(String),
    Popup(PopupMessage),
    #[default]
    Tick,
    Quit,
    Batch(Vec<Message>),
}

#[allow(dead_code)]
pub struct App {
    context: Context,
    pages: PageStack<Cid, Message>,
    theme: Theme,
    quit: bool,
}

/// Creates a new application using a tui-realm-application, mounts all
/// components and sets focus to a default one.
#[allow(dead_code)]
impl App {
    pub fn new(context: Context) -> Self {
        Self {
            context,
            pages: PageStack::default(),
            theme: theme::default_dark(),
            quit: false,
        }
    }

    fn view_list(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        theme: &Theme,
    ) -> Result<()> {
        let list = Box::new(ListPage::new(theme.clone()));
        self.pages.push(list, app, &self.context, theme)?;

        Ok(())
    }

    fn view_issue(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        id: Option<IssueId>,
        theme: &Theme,
    ) -> Result<()> {
        let repo = self.context.repository();
        match id {
            Some(id) => {
                if let Some(issue) = cob::issue::find(repo, &id)? {
                    let view = Box::new(IssuePage::new(&self.context, theme, Some((id, issue))));
                    self.pages.push(view, app, &self.context, theme)?;

                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "Could not mount 'page::IssueView'. Issue not found."
                    ))
                }
            }
            None => {
                let view = Box::new(IssuePage::new(&self.context, theme, None));
                self.pages.push(view, app, &self.context, theme)?;

                Ok(())
            }
        }
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
            Message::Issue(IssueMessage::Cob(IssueCobMessage::Create {
                title,
                tags,
                assignees,
                description,
            })) => match self.create_issue(title, description, tags, assignees) {
                Ok(id) => {
                    self.context.reload();

                    Ok(Some(Message::Batch(vec![
                        Message::Issue(IssueMessage::HideForm),
                        Message::Issue(IssueMessage::Created(id)),
                    ])))
                }
                Err(err) => {
                    let error = format!("{:?}", err);
                    self.show_error_popup(app, &theme, &error)?;

                    Ok(None)
                }
            },
            Message::Issue(IssueMessage::Show(id)) => {
                self.view_issue(app, id, &theme)?;
                Ok(None)
            }
            Message::Issue(IssueMessage::Leave(id)) => {
                self.pages.pop(app)?;
                Ok(Some(Message::Issue(IssueMessage::Reload(id))))
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
            Message::Quit => {
                self.quit = true;
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

    fn create_issue(
        &mut self,
        title: String,
        description: String,
        labels: String,
        assignees: String,
    ) -> Result<IssueId> {
        let repository = self.context.repository();
        let signer = self.context.signer();

        let labels = cob::parse_labels(labels)?;
        let assignees = cob::parse_assignees(assignees)?;

        cob::issue::create(
            repository,
            signer,
            title,
            description,
            labels.as_slice(),
            assignees.as_slice(),
        )
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
            vec![Sub::new(subscription::quit_clause(Key::Char('q')), SubClause::Always)],
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
            return Some(Exit { value: None });
        }
        None
    }
}
