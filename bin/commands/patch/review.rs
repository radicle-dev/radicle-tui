#[path = "review/builder.rs"]
pub mod builder;

use std::fmt::Debug;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;

use ratatui::text::Line;
use termion::event::Key;

use ratatui::layout::{Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::text::Text;
use ratatui::{Frame, Viewport};

use radicle::identity::RepoId;
use radicle::storage::git::Repository;
use radicle::storage::{ReadStorage, WriteRepository};
use radicle::Profile;

use radicle_cli as cli;

use cli::terminal::highlight::Highlighter;

use radicle_tui as tui;

use tui::store;
use tui::ui::im::widget::GroupState;
use tui::ui::im::widget::{TableState, TextViewState, Window};
use tui::ui::im::Ui;
use tui::ui::im::{Borders, Context, Show};
use tui::ui::span;
use tui::ui::Column;
use tui::{Channel, Exit};

use crate::ui::items::ReviewItem;
use crate::ui::items::ToText;

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
    pub queue: ReviewQueue,
}

impl Tui {
    pub fn new(profile: Profile, rid: RepoId, queue: ReviewQueue) -> Self {
        Self {
            rid,
            profile,
            queue,
        }
    }

    pub async fn run(&self) -> Result<Option<Selection>> {
        let viewport = Viewport::Fullscreen;

        let channel = Channel::default();
        let state = App::new(self.profile.clone(), self.rid, self.queue.clone())?;

        tui::im(state, viewport, channel).await
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    WindowsChanged { state: GroupState },
    ItemChanged { state: TableState },
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

#[derive(Clone)]
pub struct App<'a> {
    repository: Arc<Mutex<Repository>>,
    queue: (Vec<ReviewItem<'a>>, TableState),
    page: AppPage,
    windows: GroupState,
    help: HelpState<'a>,
}

impl<'a> TryFrom<&Tui> for App<'a> {
    type Error = anyhow::Error;

    fn try_from(tui: &Tui) -> Result<Self, Self::Error> {
        App::new(tui.profile.clone(), tui.rid, tui.queue.clone())
    }
}

impl<'a> App<'a> {
    pub fn new(profile: Profile, rid: RepoId, queue: ReviewQueue) -> Result<Self, anyhow::Error> {
        let repository = profile.storage.repository(rid)?;

        let queue = queue
            .iter()
            .map(|item| ReviewItem::from((&repository, item)))
            .collect::<Vec<_>>();

        Ok(Self {
            repository: Arc::new(Mutex::new(repository)),
            page: AppPage::Main,
            windows: GroupState::new(2, Some(0)),
            help: HelpState {
                text: TextViewState::new(help_text(), (0, 0)),
            },
            queue: (queue, TableState::new(Some(0))),
        })
    }
}

impl<'a> App<'a> {
    fn show_hunk_list(&self, ui: &mut Ui<Message>, frame: &mut Frame) {
        let columns = [
            Column::new(" ", Constraint::Length(1)),
            Column::new(" ", Constraint::Fill(1)),
            Column::new(" ", Constraint::Fill(1)),
        ]
        .to_vec();
        let mut selected = self.queue.1.selected();

        let table = ui.table(
            frame,
            &mut selected,
            &self.queue.0,
            columns,
            Some(Borders::All),
        );
        if table.changed {
            ui.send_message(Message::ItemChanged {
                state: TableState::new(selected),
            })
        }
    }

    fn show_review_item(&self, ui: &mut Ui<Message>, frame: &mut Frame) {
        let repo = self.repository.lock().unwrap();
        let mut hi = Highlighter::default();

        let selected = self.queue.1.selected();
        let item = selected.and_then(|selected| self.queue.0.get(selected));

        if let Some(item) = item {
            ui.composite(
                Layout::vertical([Constraint::Length(3), Constraint::Min(1)]),
                1,
                |ui| match &item.inner {
                    (
                        _,
                        crate::cob::ReviewItem::FileAdded {
                            path,
                            header: _,
                            new: _,
                            hunk,
                            stats: _,
                        },
                    ) => {
                        let path = ReviewItem::pretty_path(path, false);
                        let header = [
                            Column::new("", Constraint::Length(0)),
                            Column::new(path.clone(), Constraint::Length(path.width() as u16)),
                            Column::new(
                                span::default(" added ")
                                    .light_green()
                                    .dim()
                                    .reversed()
                                    .into_right_aligned_line(),
                                Constraint::Fill(1),
                            ),
                        ];
                        let hunk = hunk
                            .as_ref()
                            .map(|hunk| {
                                Text::from(hunk.to_text(&mut hi, &item.highlighted, repo.raw()))
                            })
                            .unwrap_or(Text::raw("No hunk found").light_red());

                        ui.columns(frame, header.clone().to_vec(), Some(Borders::Top));
                        ui.text_view(frame, hunk, &mut (0, 0), Some(Borders::BottomSides));
                    }
                    (
                        _,
                        crate::cob::ReviewItem::FileModified {
                            path,
                            header: _,
                            old: _,
                            new: _,
                            hunk,
                            stats: _,
                        },
                    ) => {
                        let path = ReviewItem::pretty_path(path, false);
                        let header = [
                            Column::new("", Constraint::Length(0)),
                            Column::new(path.clone(), Constraint::Length(path.width() as u16)),
                            Column::new(
                                span::default(" modified ")
                                    .light_yellow()
                                    .dim()
                                    .reversed()
                                    .into_right_aligned_line(),
                                Constraint::Fill(1),
                            ),
                        ];
                        let hunk = hunk
                            .as_ref()
                            .map(|hunk| {
                                Text::from(hunk.to_text(&mut hi, &item.highlighted, repo.raw()))
                            })
                            .unwrap_or(Text::raw("No hunk found").light_red());

                        ui.columns(frame, header.clone().to_vec(), Some(Borders::Top));
                        ui.text_view(frame, hunk, &mut (0, 0), Some(Borders::BottomSides));
                    }
                    (
                        _,
                        crate::cob::ReviewItem::FileDeleted {
                            path,
                            header: _,
                            old: _,
                            hunk,
                            stats: _,
                        },
                    ) => {
                        let path = ReviewItem::pretty_path(path, true);
                        let header = [
                            Column::new("", Constraint::Length(0)),
                            Column::new(path.clone(), Constraint::Length(path.width() as u16)),
                            Column::new(
                                span::default(" deleted ")
                                    .light_red()
                                    .dim()
                                    .reversed()
                                    .into_right_aligned_line(),
                                Constraint::Fill(1),
                            ),
                        ];
                        let hunk = hunk
                            .as_ref()
                            .map(|hunk| {
                                Text::from(hunk.to_text(&mut hi, &item.highlighted, repo.raw()))
                            })
                            .unwrap_or(Text::raw("No hunk found").light_red());

                        ui.columns(frame, header.clone().to_vec(), Some(Borders::Top));
                        ui.text_view(frame, hunk, &mut (0, 0), Some(Borders::BottomSides));
                    }
                    (_, crate::cob::ReviewItem::FileCopied { copied }) => {
                        let path = Line::from(
                            [
                                ReviewItem::pretty_path(&copied.old_path, false).spans,
                                [span::default(" -> ")].to_vec(),
                                ReviewItem::pretty_path(&copied.new_path, false).spans,
                            ]
                            .concat()
                            .to_vec(),
                        );
                        let header = [
                            Column::new("", Constraint::Length(0)),
                            Column::new(path.clone(), Constraint::Length(path.width() as u16)),
                            Column::new(
                                span::default(" copied ")
                                    .light_red()
                                    .dim()
                                    .reversed()
                                    .into_right_aligned_line(),
                                Constraint::Fill(1),
                            ),
                        ];
                        ui.columns(frame, header.clone().to_vec(), Some(Borders::Top));
                    }
                    (_, crate::cob::ReviewItem::FileMoved { moved }) => {
                        let path = Line::from(
                            [
                                ReviewItem::pretty_path(&moved.old_path, false).spans,
                                [span::default(" -> ")].to_vec(),
                                ReviewItem::pretty_path(&moved.new_path, false).spans,
                            ]
                            .concat()
                            .to_vec(),
                        );
                        let header = [
                            Column::new("", Constraint::Length(0)),
                            Column::new(path.clone(), Constraint::Length(path.width() as u16)),
                            Column::new(
                                span::default(" moved ")
                                    .light_blue()
                                    .dim()
                                    .reversed()
                                    .into_right_aligned_line(),
                                Constraint::Fill(1),
                            ),
                        ];
                        ui.columns(frame, header.clone().to_vec(), Some(Borders::All));
                    }
                    (
                        _,
                        crate::cob::ReviewItem::FileEofChanged {
                            path,
                            header: _,
                            old: _,
                            new: _,
                            eof: _,
                        },
                    ) => {
                        let path = ReviewItem::pretty_path(&path, false);
                        let header = [
                            Column::new("", Constraint::Length(0)),
                            Column::new(path.clone(), Constraint::Length(path.width() as u16)),
                            Column::new(
                                span::default(" eof ")
                                    .dim()
                                    .reversed()
                                    .into_right_aligned_line(),
                                Constraint::Fill(1),
                            ),
                        ];
                        ui.columns(frame, header.clone().to_vec(), Some(Borders::All));
                    }
                    (
                        _,
                        crate::cob::ReviewItem::FileModeChanged {
                            path,
                            header: _,
                            old: _,
                            new: _,
                        },
                    ) => {
                        let path = ReviewItem::pretty_path(&path, false);
                        let header = [
                            Column::new("", Constraint::Length(0)),
                            Column::new(path.clone(), Constraint::Length(path.width() as u16)),
                            Column::new(
                                span::default(" mode ")
                                    .dim()
                                    .reversed()
                                    .into_right_aligned_line(),
                                Constraint::Length(6),
                            ),
                        ];
                        ui.columns(frame, header.clone().to_vec(), Some(Borders::All));
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

impl<'a> store::Update<Message> for App<'a> {
    type Return = Selection;

    fn update(&mut self, message: Message) -> Option<Exit<Self::Return>> {
        log::info!("Received message: {:?}", message);
        match message {
            Message::WindowsChanged { state } => {
                self.windows = state;
                None
            }
            Message::ItemChanged { state } => {
                self.queue.1 = state;
                None
            }
            Message::Quit => Some(Exit { value: None }),
            Message::Accept => Some(Exit {
                value: Some(Selection {
                    action: ReviewAction::Accept,
                    hunk: self.queue.1.selected(),
                    args: None,
                }),
            }),
            Message::Comment => Some(Exit {
                value: Some(Selection {
                    action: ReviewAction::Comment,
                    hunk: self.queue.1.selected(),
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
