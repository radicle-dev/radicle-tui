use anyhow::Result;

use termion::event::Key;

use ratatui::layout::{Constraint, Layout};
use ratatui::{Frame, Viewport};

use radicle::patch::PatchId;
use radicle::storage::git::Repository;
use radicle::Profile;

use radicle_tui as tui;

use tui::store;
use tui::ui::im::widget::{TextViewState, Window};
use tui::ui::im::{Borders, Context, Show};
use tui::{Channel, Exit};

use crate::cob::patch;
use crate::ui::items::PatchItem;

type Selection = tui::Selection<PatchId>;

pub struct Tui {
    pub profile: Profile,
    pub repository: Repository,
}

impl Tui {
    pub fn new(profile: Profile, repository: Repository) -> Self {
        Self {
            profile,
            repository,
        }
    }

    pub async fn run(&self) -> Result<Option<Selection>> {
        let viewport = Viewport::Inline(20);

        let channel = Channel::default();
        let state = App::try_from(self)?;

        tui::im(state, viewport, channel).await
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    Quit,
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

impl TryFrom<&Tui> for App {
    type Error = anyhow::Error;

    fn try_from(tui: &Tui) -> Result<Self, Self::Error> {
        let patches = patch::all(&tui.profile, &tui.repository)?;

        // Convert into UI items
        let mut items = vec![];
        for patch in patches {
            if let Ok(item) = PatchItem::new(&tui.profile, &tui.repository, patch.clone()) {
                items.push(item);
            }
        }
        items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

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

    fn update(&mut self, message: Message) -> Option<Exit<Selection>> {
        match message {
            Message::Quit => Some(Exit { value: None }),
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
                            ui.text_view(frame, String::from("Review"), &mut (0, 0), Some(Borders::All));
                            ui.shortcuts(frame, &[("q", "quit"), ("?", "help")], '∙');
                        },
                    );

                    if ui.input_global(|key| key == Key::Char('?')) {
                        ui.send_message(Message::ShowHelp);
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
                            ui.shortcuts(frame, &[("q", "quit"), ("?", "close")], '∙');
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
