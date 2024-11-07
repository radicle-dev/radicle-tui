#[path = "review/builder.rs"]
pub mod builder;

use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;

use ratatui::layout::Position;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::text::Text;
use ratatui::{Frame, Viewport};
use termion::event::Key;

use radicle::identity::RepoId;
use radicle::patch::Review;
use radicle::storage::ReadStorage;
use radicle::Profile;

use radicle_tui as tui;

use tui::store;
use tui::ui::im::widget::GroupState;
use tui::ui::im::widget::{TableState, TextViewState, Window};
use tui::ui::im::Ui;
use tui::ui::im::{Borders, Context, Show};
use tui::ui::Column;
use tui::{Channel, Exit};

use crate::ui::items::HunkItem;

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
    pub hunk: Option<usize>,
    pub args: Option<Args>,
}

pub struct Tui {
    pub profile: Profile,
    pub rid: RepoId,
    pub review: Review,
    pub queue: ReviewQueue,
}

impl Tui {
    pub fn new(profile: Profile, rid: RepoId, review: Review, queue: ReviewQueue) -> Self {
        Self {
            rid,
            profile,
            review,
            queue,
        }
    }

    pub async fn run(&self) -> Result<Option<Selection>> {
        let viewport = Viewport::Fullscreen;

        let channel = Channel::default();
        let state = App::new(
            self.profile.clone(),
            self.rid,
            self.review.clone(),
            self.queue.clone(),
        )?;

        tui::im(state, viewport, channel).await
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    WindowsChanged { state: GroupState },
    ItemChanged { state: TableState },
    ItemViewChanged { state: ReviewItemState },
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
pub struct HelpState<'a> {
    text: TextViewState<'a>,
}

#[derive(Clone, Debug)]
pub struct ReviewItemState {
    cursor: Position,
}

#[derive(Clone)]
pub struct App<'a> {
    queue: Arc<Mutex<(Vec<HunkItem<'a>>, TableState)>>,
    items: HashMap<usize, ReviewItemState>,
    page: AppPage,
    windows: GroupState,
    help: HelpState<'a>,
}

impl<'a> TryFrom<&Tui> for App<'a> {
    type Error = anyhow::Error;

    fn try_from(tui: &Tui) -> Result<Self, Self::Error> {
        App::new(
            tui.profile.clone(),
            tui.rid,
            tui.review.clone(),
            tui.queue.clone(),
        )
    }
}

impl<'a> App<'a> {
    pub fn new(
        profile: Profile,
        rid: RepoId,
        review: Review,
        queue: ReviewQueue,
    ) -> Result<Self, anyhow::Error> {
        let repository = profile.storage.repository(rid)?;

        let queue = queue
            .iter()
            .map(|item| HunkItem::from((&repository, &review, item)))
            .collect::<Vec<_>>();

        let mut items = HashMap::new();
        for (idx, _) in queue.iter().enumerate() {
            items.insert(
                idx,
                ReviewItemState {
                    cursor: Position::new(0, 0),
                },
            );
        }

        Ok(Self {
            page: AppPage::Main,
            windows: GroupState::new(2, Some(0)),
            help: HelpState {
                text: TextViewState::new(help_text(), Position::default()),
            },
            queue: Arc::new(Mutex::new((queue, TableState::new(Some(0))))),
            items,
        })
    }
}

impl<'a> App<'a> {
    fn show_hunk_list(&self, ui: &mut Ui<Message>, frame: &mut Frame) {
        let queue = self.queue.lock().unwrap();

        let columns = [
            Column::new(" ", Constraint::Length(1)),
            Column::new(" ", Constraint::Fill(1)),
            Column::new(" ", Constraint::Length(15)),
        ]
        .to_vec();
        let mut selected = queue.1.selected();

        let table = ui.table(frame, &mut selected, &queue.0, columns, Some(Borders::All));
        if table.changed {
            ui.send_message(Message::ItemChanged {
                state: TableState::new(selected),
            })
        }
    }

    fn show_review_item(&self, ui: &mut Ui<Message>, frame: &mut Frame) {
        let queue = self.queue.lock().unwrap();

        let selected = queue.1.selected();
        let item = selected.and_then(|selected| queue.0.get(selected));

        if let Some(item) = item {
            let header = item.header();
            let hunk = item
                .hunk_text()
                .unwrap_or(Text::raw("Nothing to show.").dark_gray());

            let mut cursor = selected
                .and_then(|selected| self.items.get(&selected))
                .map(|state| state.cursor)
                .unwrap_or_default();

            ui.composite(
                Layout::vertical([Constraint::Length(3), Constraint::Min(1)]),
                1,
                |ui| {
                    ui.columns(frame, header, Some(Borders::Top));

                    if let Some(hunk) = item.hunk_text() {
                        let diff =
                            ui.text_view(frame, hunk, &mut cursor, Some(Borders::BottomSides));
                        if diff.changed {
                            ui.send_message(Message::ItemViewChanged {
                                state: ReviewItemState { cursor },
                            })
                        }
                    } else {
                        ui.centered_text_view(frame, hunk, Some(Borders::BottomSides));
                    }
                },
            );
        }
    }
}

impl<'a> Show<Message> for App<'a> {
    fn show(&self, ctx: &Context<Message>, frame: &mut Frame) -> Result<(), anyhow::Error> {
        Window::default().show(ctx, |ui| {
            let mut page_focus = self.windows.focus();

            match self.page {
                AppPage::Main => {
                    ui.layout(
                        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]),
                        Some(0),
                        |ui| {
                            let group = ui.group(
                                Layout::horizontal([
                                    Constraint::Ratio(1, 3),
                                    Constraint::Ratio(2, 3),
                                ]),
                                &mut page_focus,
                                |ui| {
                                    self.show_hunk_list(ui, frame);
                                    self.show_review_item(ui, frame);
                                },
                            );
                            if group.response.changed {
                                ui.send_message(Message::WindowsChanged {
                                    state: GroupState::new(self.windows.len(), page_focus),
                                });
                            }

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

                            if ui.input_global(|key| key == Key::Char('?')) {
                                ui.send_message(Message::ShowHelp);
                            }
                            if ui.input_global(|key| key == Key::Char('a')) {
                                ui.send_message(Message::Accept);
                            }
                            if ui.input_global(|key| key == Key::Char('c')) {
                                ui.send_message(Message::Comment);
                            }
                        },
                    );
                }
                AppPage::Help => {
                    ui.group(
                        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]),
                        &mut page_focus,
                        |ui| {
                            ui.text_view(
                                frame,
                                self.help.text.text().to_string(),
                                &mut Position::default(),
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

impl<'a> store::Update<Message> for App<'a> {
    type Return = Selection;

    fn update(&mut self, message: Message) -> Option<Exit<Self::Return>> {
        log::info!("Received message: {:?}", message);

        let mut queue = self.queue.lock().unwrap();
        match message {
            Message::WindowsChanged { state } => {
                self.windows = state;
                None
            }
            Message::ItemChanged { state } => {
                queue.1 = state;
                None
            }
            Message::ItemViewChanged { state } => {
                if let Some(selected) = queue.1.selected() {
                    self.items.insert(selected, state);
                }
                None
            }
            Message::Quit => Some(Exit { value: None }),
            Message::Accept => Some(Exit {
                value: Some(Selection {
                    action: ReviewAction::Accept,
                    hunk: queue.1.selected(),
                    args: None,
                }),
            }),
            Message::Comment => Some(Exit {
                value: Some(Selection {
                    action: ReviewAction::Comment,
                    hunk: queue.1.selected(),
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
