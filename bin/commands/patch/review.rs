#[path = "review/builder.rs"]
pub mod builder;

use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;

use termion::event::Key;

use ratatui::layout::Position;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::text::Text;
use ratatui::{Frame, Viewport};

use radicle::identity::RepoId;
use radicle::patch::PatchId;
use radicle::patch::Review;
use radicle::patch::Revision;
use radicle::storage::ReadStorage;
use radicle::storage::WriteRepository;
use radicle::Profile;

use radicle_cli::terminal;

use radicle_tui as tui;

use tui::store;
use tui::ui::im::widget::GroupState;
use tui::ui::im::widget::{TableState, TextViewState, Window};
use tui::ui::im::Ui;
use tui::ui::im::{Borders, Context, Show};
use tui::ui::span;
use tui::ui::Column;
use tui::{Channel, Exit};

use crate::cob::HunkState;
use crate::cob::StatefulHunkItem;
use crate::tui_patch::review::builder::DiffUtil;
use crate::ui::format;
use crate::ui::items::HunkItem;

use self::builder::Brain;
use self::builder::FileReviewBuilder;
use self::builder::ReviewQueue;

/// The actions that a user can carry out on a review item.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReviewAction {
    Comment,
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
    pub patch: PatchId,
    pub title: String,
    pub revision: Revision,
    pub review: Review,
    pub queue: ReviewQueue,
    pub profile: Profile,
    pub rid: RepoId,
}

impl Tui {
    pub fn new(
        patch: PatchId,
        title: String,
        revision: Revision,
        review: Review,
        queue: ReviewQueue,
        profile: Profile,
        rid: RepoId,
    ) -> Self {
        Self {
            patch,
            title,
            revision,
            review,
            queue,
            rid,
            profile,
        }
    }

    pub async fn run(&self) -> Result<Option<Selection>> {
        let viewport = Viewport::Fullscreen;
        let _ = self.profile.signer()?;

        let channel = Channel::default();
        let state = App::new(
            self.patch,
            self.title.clone(),
            self.revision.clone(),
            self.review.clone(),
            self.queue.clone(),
            self.profile.clone(),
            self.rid,
        )?;

        tui::im(state, viewport, channel).await
    }
}

#[derive(Clone, Debug)]
pub enum Message<'a> {
    ShowMain,
    WindowsChanged { state: GroupState },
    ItemChanged { state: TableState },
    ItemViewChanged { state: ReviewItemState },
    Quit,
    Comment,
    Accept,
    Discard,
    ShowHelp,
    HelpChanged { state: TextViewState<'a> },
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum AppPage {
    Main,
    Help,
}

#[derive(Clone, Debug)]
pub struct ReviewItemState {
    cursor: Position,
}

#[derive(Clone)]
pub struct App<'a> {
    patch: PatchId,
    title: String,
    revision: Revision,
    queue: Arc<Mutex<(Vec<HunkItem<'a>>, TableState)>>,
    items: HashMap<usize, ReviewItemState>,
    profile: Profile,
    rid: RepoId,
    page: AppPage,
    windows: GroupState,
    help: TextViewState<'a>,
}

impl<'a> TryFrom<&Tui> for App<'a> {
    type Error = anyhow::Error;

    fn try_from(tui: &Tui) -> Result<Self, Self::Error> {
        App::new(
            tui.patch,
            tui.title.clone(),
            tui.revision.clone(),
            tui.review.clone(),
            tui.queue.clone(),
            tui.profile.clone(),
            tui.rid,
        )
    }
}

impl<'a> App<'a> {
    pub fn new(
        patch: PatchId,
        title: String,
        revision: Revision,
        review: Review,
        queue: ReviewQueue,
        profile: Profile,
        rid: RepoId,
    ) -> Result<Self, anyhow::Error> {
        let repo = profile.storage.repository(rid)?;
        let queue = queue
            .iter()
            .map(|(_, item, state)| {
                HunkItem::from((&repo, &review, StatefulHunkItem::from((item, state))))
            })
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

        let mut app = App {
            profile,
            rid,
            patch,
            title,
            revision,
            queue: Arc::new(Mutex::new((queue, TableState::new(Some(0))))),
            items,
            page: AppPage::Main,
            windows: GroupState::new(2, Some(0)),
            help: TextViewState::new(help_text(), Position::default()),
        };

        app.reload_states()?;

        Ok(app)
    }

    pub fn accept_current_hunk(&mut self) -> Result<()> {
        let repo = self.profile.storage.repository(self.rid).unwrap();
        let signer = terminal::signer(&self.profile)?;

        let mut brain = Brain::load_or_new(self.patch, &self.revision, repo.raw(), &signer)?;
        let selected = self.queue.lock().unwrap().1.selected();

        if let Some(selected) = selected {
            let hunks = self.queue.lock().unwrap().0.clone();

            if let Some(hunk) = hunks.get(selected) {
                let mut file: Option<FileReviewBuilder> = None;
                let file = match file.as_mut() {
                    Some(fr) => fr.set_item(hunk.inner.hunk()),
                    None => file.insert(FileReviewBuilder::new(hunk.inner.hunk())),
                };

                let diff = file.item_diff(hunk.inner.hunk())?;
                brain.accept(diff, repo.raw())?;

                self.reload_states()?;
            }
        }

        Ok(())
    }

    pub fn discard_accepted_hunks(&mut self) -> Result<()> {
        let repo = self.profile.storage.repository(self.rid).unwrap();
        let signer = terminal::signer(&self.profile)?;

        let mut brain = Brain::load_or_new(self.patch, &self.revision, repo.raw(), &signer)?;
        brain.discard_accepted(repo.raw())?;

        self.reload_states()?;

        Ok(())
    }

    pub fn reload_states(&mut self) -> anyhow::Result<()> {
        let repo = self.profile.storage.repository(self.rid).unwrap();
        let signer = terminal::signer(&self.profile)?;

        let brain = Brain::load_or_new(self.patch, &self.revision, repo.raw(), &signer)?;

        let (base_diff, queue_diff) =
            DiffUtil::new(&repo).base_queue(brain.clone(), &self.revision)?;

        // Compute states
        let base_files = base_diff.into_files();
        let queue_files = queue_diff.into_files();

        let states = base_files
            .iter()
            .map(|file| {
                if !queue_files.contains(file) {
                    HunkState::Accepted
                } else {
                    HunkState::Rejected
                }
            })
            .collect::<Vec<_>>();

        let mut queue = self.queue.lock().unwrap();
        for (idx, new_state) in states.iter().enumerate() {
            if let Some(hunk) = queue.0.get_mut(idx) {
                *hunk.inner.state_mut() = new_state.clone();
            }
        }

        Ok(())
    }
}

impl<'a> App<'a> {
    fn show_hunk_list(&self, ui: &mut Ui<Message<'a>>, frame: &mut Frame) {
        let header = [Column::new(" Hunks ", Constraint::Fill(1))].to_vec();
        let columns = [
            Column::new("", Constraint::Length(2)),
            Column::new("", Constraint::Fill(1)),
            Column::new("", Constraint::Length(15)),
        ]
        .to_vec();

        let queue = self.queue.lock().unwrap();
        let mut selected = queue.1.selected();

        let table = ui.headered_table(frame, &mut selected, &queue.0, header, columns);
        if table.changed {
            ui.send_message(Message::ItemChanged {
                state: TableState::new(selected),
            })
        }
    }

    fn show_review_item(&self, ui: &mut Ui<Message<'a>>, frame: &mut Frame) {
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

    fn show_context_bar(&self, ui: &mut Ui<Message<'a>>, frame: &mut Frame) {
        let queue = &self.queue.lock().unwrap().0;

        let id = format!(" {} ", format::cob(&self.patch));
        let title = &self.title;

        let hunks_total = queue.len();
        let hunks_accepted = queue
            .iter()
            .filter(|item| *item.inner.state() == HunkState::Accepted)
            .collect::<Vec<_>>()
            .len();

        let accepted_stats = format!(" Accepted {hunks_accepted}/{hunks_total} ");

        ui.bar(
            frame,
            [
                Column::new(
                    span::default(" Review ").cyan().dim().reversed(),
                    Constraint::Length(8),
                ),
                Column::new(
                    span::default(&id)
                        .style(ui.theme().bar_on_black_style)
                        .magenta(),
                    Constraint::Length(9),
                ),
                Column::new(
                    span::default(title)
                        .style(ui.theme().bar_on_black_style)
                        .magenta()
                        .dim(),
                    Constraint::Length(title.chars().count() as u16),
                ),
                Column::new(
                    span::default(" ")
                        .into_left_aligned_line()
                        .style(ui.theme().bar_on_black_style),
                    Constraint::Fill(1),
                ),
                Column::new(
                    span::default(&accepted_stats)
                        .into_right_aligned_line()
                        .cyan()
                        .dim()
                        .reversed(),
                    Constraint::Length(accepted_stats.chars().count() as u16),
                ),
            ]
            .to_vec(),
            Some(Borders::None),
        );
    }
}

impl<'a> Show<Message<'a>> for App<'a> {
    fn show(&self, ctx: &Context<Message<'a>>, frame: &mut Frame) -> Result<(), anyhow::Error> {
        Window::default().show(ctx, |ui| {
            let mut page_focus = self.windows.focus();

            match self.page {
                AppPage::Main => {
                    ui.layout(
                        Layout::vertical([
                            Constraint::Fill(1),
                            Constraint::Length(1),
                            Constraint::Length(1),
                        ]),
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

                            self.show_context_bar(ui, frame);

                            ui.shortcuts(
                                frame,
                                &[
                                    ("c", "comment"),
                                    ("a", "accept"),
                                    ("d", "discard accepted"),
                                    ("?", "help"),
                                    ("q", "quit"),
                                ],
                                '∙',
                            );

                            if ui.input_global(|key| key == Key::Char('?')) {
                                ui.send_message(Message::ShowHelp);
                            }
                            if ui.input_global(|key| key == Key::Char('c')) {
                                ui.send_message(Message::Comment);
                            }
                            if ui.input_global(|key| key == Key::Char('a')) {
                                ui.send_message(Message::Accept);
                            }
                            if ui.input_global(|key| key == Key::Char('d')) {
                                ui.send_message(Message::Discard);
                            }
                        },
                    );
                }
                AppPage::Help => {
                    ui.group(
                        Layout::vertical([
                            Constraint::Fill(1),
                            Constraint::Length(1),
                            Constraint::Length(1),
                        ]),
                        &mut page_focus,
                        |ui| {
                            ui.composite(
                                Layout::vertical([Constraint::Length(3), Constraint::Min(1)]),
                                1,
                                |ui| {
                                    let header =
                                        [Column::new(" Help ", Constraint::Fill(1))].to_vec();
                                    let mut cursor = self.help.cursor();

                                    ui.columns(frame, header, Some(Borders::Top));
                                    let help = ui.text_view(
                                        frame,
                                        self.help.text().to_string(),
                                        &mut cursor,
                                        Some(Borders::BottomSides),
                                    );
                                    if help.changed {
                                        ui.send_message(Message::HelpChanged {
                                            state: TextViewState::new(
                                                self.help.text().clone(),
                                                cursor,
                                            ),
                                        })
                                    }
                                },
                            );

                            self.show_context_bar(ui, frame);

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

impl<'a> store::Update<Message<'a>> for App<'a> {
    type Return = Selection;

    fn update(&mut self, message: Message<'a>) -> Option<Exit<Self::Return>> {
        log::info!("Received message: {:?}", message);

        match message {
            Message::WindowsChanged { state } => {
                self.windows = state;
                None
            }
            Message::ItemChanged { state } => {
                let mut queue = self.queue.lock().unwrap();
                queue.1 = state;
                None
            }
            Message::ItemViewChanged { state } => {
                let queue = self.queue.lock().unwrap();
                if let Some(selected) = queue.1.selected() {
                    self.items.insert(selected, state);
                }
                None
            }
            Message::Quit => Some(Exit { value: None }),
            Message::Comment => {
                let queue = self.queue.lock().unwrap();
                Some(Exit {
                    value: Some(Selection {
                        action: ReviewAction::Comment,
                        hunk: queue.1.selected(),
                        args: None,
                    }),
                })
            }
            Message::Accept => {
                match self.accept_current_hunk() {
                    Ok(()) => log::info!("Accepted hunk."),
                    Err(err) => log::info!("An error occured while accepting hunk: {}", err),
                }
                None
            }
            Message::Discard => {
                match self.discard_accepted_hunks() {
                    Ok(()) => log::info!("Discarded all hunks."),
                    Err(err) => log::info!("An error occured while discarding hunks: {}", err),
                }
                None
            }
            Message::ShowMain => {
                self.page = AppPage::Main;
                None
            }
            Message::ShowHelp => {
                self.page = AppPage::Help;
                None
            }
            Message::HelpChanged { state } => {
                self.help = state;
                None
            }
        }
    }
}

fn help_text() -> String {
    r#"# About

A terminal interface for reviewing patch revisions.

Starts a new or resumes an existing review for a given revision (default: latest). When the
review is done, it needs to be finalized via `rad patch review --accept | --reject <id>`.
    
# Keybindings

`←,h`       move cursor to the left
`↑,k`       move cursor one line up
`↓,j`       move cursor one line down
`→,l`       move cursor to the right
`PageUp`    move cursor one page up
`PageDown`  move cursor one page down
`Home`      move cursor to the first line
`End`       move cursor to the last line

`?`         toogle help
`q`         quit / cancel

## Specific keybindings

`c`         comment on hunk
`a`         accept hunk
`d`         discard accepted hunks (reject all)"#
        .into()
}
