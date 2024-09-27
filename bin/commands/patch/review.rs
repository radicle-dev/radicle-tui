#[path = "review/builder.rs"]
pub mod builder;

use anyhow::Result;

use termion::event::Key;

use ratatui::layout::{Constraint, Layout};
use ratatui::{Frame, Viewport};

use radicle::storage::git::Repository;
use radicle::Profile;

use radicle_tui as tui;

use tui::store;
use tui::ui::im::widget::{TextViewState, Window};
use tui::ui::im::{Borders, Context, Show};
use tui::{Channel, Exit};

use self::builder::ReviewQueue;

/// The actions that a user can carry out on a review item.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReviewAction {
    Accept,
    Ignore,
    Comment,
}

impl std::fmt::Display for ReviewAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Accept => write!(f, "accept"),
            Self::Ignore => write!(f, "ignore"),
            Self::Comment => write!(f, "comment"),
        }
    }
}

impl TryFrom<&str> for ReviewAction {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "accept" => Ok(ReviewAction::Accept),
            "ignore" => Ok(ReviewAction::Ignore),
            "comment" => Ok(ReviewAction::Comment),
            _ => anyhow::bail!("Unknown review action"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Args(String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Selection {
    pub action: ReviewAction,
    pub hunk: usize,
    pub args: Option<Args>,
}

pub struct Tui<'a> {
    pub _profile: &'a Profile,
    pub _repository: &'a Repository,
    pub queue: &'a ReviewQueue,
}

impl<'a> Tui<'a> {
    pub fn new(profile: &'a Profile, repository: &'a Repository, queue: &'a ReviewQueue) -> Self {
        Self {
            _profile: profile,
            _repository: repository,
            queue,
        }
    }

    pub async fn run(&self) -> Result<Option<Selection>> {
        let viewport = Viewport::Fullscreen;

        let channel = Channel::default();
        let state = App::try_from(self)?;

        tui::im(state, viewport, channel).await
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    Quit,
    Accept,
    Comment,
    ShowMain,
    ShowHelp,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum AppPage {
    Main,
    Help,
}

#[derive(Clone, Debug)]
pub struct HelpState {
    text: TextViewState,
}

#[derive(Clone, Debug)]
pub struct App {
    page: AppPage,
    help: HelpState,
}

impl<'a> TryFrom<&Tui<'a>> for App {
    type Error = anyhow::Error;

    fn try_from(_tui: &Tui) -> Result<Self, Self::Error> {
        Ok(Self {
            page: AppPage::Main,
            help: HelpState {
                text: TextViewState::new(help_text(), (0, 0)),
            },
        })
    }
}

impl store::Update<Message> for App {
    type Return = Selection;

    fn update(&mut self, message: Message) -> Option<Exit<Self::Return>> {
        match message {
            Message::Quit => Some(Exit { value: None }),
            Message::Accept => Some(Exit {
                value: Some(Selection {
                    action: ReviewAction::Accept,
                    hunk: 0,
                    args: None,
                }),
            }),
            Message::Comment => Some(Exit {
                value: Some(Selection {
                    action: ReviewAction::Comment,
                    hunk: 0,
                    args: None,
                }),
            }),
            Message::ShowMain => {
                self.page = AppPage::Main;
                None
            }
            Message::ShowHelp => {
                self.page = AppPage::Help;
                None
            }
        }
    }
}

impl Show<Message> for App {
    fn show(&self, ctx: &Context<Message>, frame: &mut Frame) -> Result<()> {
        Window::default().show(ctx, |ui| {
            let mut page_focus = Some(0);

            match self.page {
                AppPage::Main => {
                    ui.group(
                        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]),
                        &mut page_focus,
                        |ui| {
                            ui.text_view(
                                frame,
                                String::from("Review"),
                                &mut (0, 0),
                                Some(Borders::All),
                            );
                            ui.shortcuts(
                                frame,
                                &[
                                    ("a", "accept"),
                                    ("c", "comment"),
                                    ("?", "help"),
                                    ("q", "quit"),
                                ],
                                '∙',
                            );
                        },
                    );

                    if ui.input_global(|key| key == Key::Char('?')) {
                        ui.send_message(Message::ShowHelp);
                    }
                    if ui.input_global(|key| key == Key::Char('a')) {
                        ui.send_message(Message::Accept);
                    }
                    if ui.input_global(|key| key == Key::Char('c')) {
                        ui.send_message(Message::Comment);
                    }
                }
                AppPage::Help => {
                    ui.group(
                        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]),
                        &mut page_focus,
                        |ui| {
                            ui.text_view(
                                frame,
                                self.help.text.text().to_string(),
                                &mut (0, 0),
                                Some(Borders::All),
                            );
                            ui.shortcuts(frame, &[("?", "close"), ("q", "quit")], '∙');
                        },
                    );

                    if ui.input_global(|key| key == Key::Char('?')) {
                        ui.send_message(Message::ShowMain);
                    }
                }
            }

            if ui.input_global(|key| key == Key::Char('q')) {
                ui.send_message(Message::Quit);
            }
        });
        Ok(())
    }
}

fn help_text() -> String {
    r#"# Generic keybindings

`↑,k`:      move cursor one line up
`↓,j:       move cursor one line down
`PageUp`:   move cursor one page up
`PageDown`: move cursor one page down
`Home`:     move cursor to the first line
`End`:      move cursor to the last line
`Esc`:      Quit / cancel"#
        .into()
}
