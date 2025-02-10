#[path = "review/builder.rs"]
pub mod builder;

use std::fmt::Debug;
use std::sync::{Arc, Mutex};

use anyhow::Result;

use serde::{Deserialize, Serialize};

use termion::event::Key;

use ratatui::layout::{Constraint, Position};
use ratatui::style::Stylize;
use ratatui::text::Text;
use ratatui::{Frame, Viewport};

use radicle::identity::RepoId;
use radicle::patch::{PatchId, Review, Revision};
use radicle::storage::ReadStorage;
use radicle::Storage;

use radicle_tui as tui;

use tui::store;
use tui::ui::im::widget::{PanesState, TableState, TextViewState, Window};
use tui::ui::im::{Borders, Context, Show, Ui};
use tui::ui::span;
use tui::ui::Column;
use tui::{Channel, Exit};

use crate::git::HunkState;
use crate::state::{Decode, Encode, FileStorage, Filename, ReadState, WriteState};
use crate::ui::format;
use crate::ui::items::HunkItem;
use crate::ui::items::StatefulHunkItem;
use crate::ui::layout;

use self::builder::Hunks;

/// The actions that a user can carry out on a review item.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReviewAction {
    Comment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Args(String);

#[derive(Clone, Debug)]
pub struct Response {
    pub state: AppState,
    pub action: Option<ReviewAction>,
}

#[derive(Clone)]
pub enum ReviewMode {
    Create,
    Resume,
}

pub struct Tui {
    pub mode: ReviewMode,
    pub storage: Storage,
    pub rid: RepoId,
    pub patch: PatchId,
    pub title: String,
    pub revision: Revision,
    pub review: Review,
    pub hunks: Hunks,
}

impl Tui {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mode: ReviewMode,
        storage: Storage,
        rid: RepoId,
        patch: PatchId,
        title: String,
        revision: Revision,
        review: Review,
        hunks: Hunks,
    ) -> Self {
        Self {
            mode,
            storage,
            rid,
            patch,
            title,
            revision,
            review,
            hunks,
        }
    }

    pub async fn run(self) -> Result<Option<Response>> {
        let viewport = Viewport::Fullscreen;
        let channel = Channel::default();

        let state_file = Filename::new("patch", "review", &self.patch.to_string());
        let state_storage = FileStorage::new(state_file)?;

        let state = AppState::new(
            self.rid,
            self.patch,
            self.title,
            self.revision,
            AppPage::Main,
            PanesState::new(2, Some(0)),
            (
                TableState::new(Some(0)),
                vec![HunkState::Rejected; self.hunks.len()],
            ),
            vec![DiffViewState::default(); self.hunks.len()],
            TextViewState::new(Position::default()),
        );

        let state = match self.mode {
            ReviewMode::Resume => match state_storage.read() {
                Ok(bytes) => AppState::from_json(&bytes)?,
                _ => {
                    log::warn!("Failed to load state. Falling back to default.");
                    state
                }
            },
            ReviewMode::Create => state,
        };

        let app = App::new(self.mode, self.storage, self.review, self.hunks, state)?;
        let response = tui::im(app, viewport, channel).await?;

        if let Some(response) = response.as_ref() {
            state_storage.write(&response.state.clone().to_json()?)?;
        }

        Ok(response)
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    ShowMain,
    PanesChanged { state: PanesState },
    HunkChanged { state: TableState },
    HunkViewChanged { state: DiffViewState },
    ShowHelp,
    HelpChanged { state: TextViewState },
    Comment,
    Accept,
    Reject,
    Quit,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum AppPage {
    Main,
    Help,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct DiffViewState {
    cursor: Position,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppState {
    /// The repository to operate on.
    rid: RepoId,
    /// Patch this review belongs to.
    patch: PatchId,
    /// Patch title.
    title: String,
    /// Revision this review belongs to.
    _revision: Revision,
    /// Current app page.
    page: AppPage,
    /// State of panes widget on the main page.
    panes: PanesState,
    /// The hunks' table widget state.
    hunks: (TableState, Vec<HunkState>),
    /// Diff view states (cursor position is stored per hunk)
    views: Vec<DiffViewState>,
    /// State of text view widget on the help page.
    help: TextViewState,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rid: RepoId,
        patch: PatchId,
        title: String,
        revision: Revision,
        page: AppPage,
        panes: PanesState,
        hunks: (TableState, Vec<HunkState>),
        views: impl IntoIterator<Item = DiffViewState>,
        help: TextViewState,
    ) -> Self {
        Self {
            rid,
            patch,
            title,
            _revision: revision,
            page,
            panes,
            hunks,
            views: views.into_iter().collect(),
            help,
        }
    }

    pub fn view_state(&self, index: usize) -> Option<&DiffViewState> {
        self.views.get(index)
    }

    pub fn update_view_state(&mut self, index: usize, state: DiffViewState) {
        if let Some(view) = self.views.get_mut(index) {
            *view = state;
        }
    }

    pub fn update_hunks(&mut self, hunks: TableState) {
        self.hunks.0 = hunks;
    }

    pub fn selected_hunk(&self) -> Option<usize> {
        self.hunks.0.selected()
    }

    pub fn accept_hunk(&mut self, index: usize) {
        if let Some(state) = self.hunks.1.get_mut(index) {
            *state = HunkState::Accepted;
        }
    }

    pub fn reject_hunk(&mut self, index: usize) {
        if let Some(state) = self.hunks.1.get_mut(index) {
            *state = HunkState::Rejected;
        }
    }

    pub fn hunk_states(&self) -> &Vec<HunkState> {
        &self.hunks.1
    }
}

#[derive(Clone)]
pub struct App<'a> {
    /// All hunks.
    hunks: Arc<Mutex<Vec<HunkItem<'a>>>>,
    /// The app state.
    state: AppState,
    /// Review mode: create or resume.
    _mode: ReviewMode,
}

impl<'a> App<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mode: ReviewMode,
        storage: Storage,
        review: Review,
        hunks: Hunks,
        state: AppState,
    ) -> Result<Self, anyhow::Error> {
        let repo = storage.repository(state.rid)?;
        let hunks = hunks
            .iter()
            .map(|item| HunkItem::from((&repo, &review, item)))
            .collect::<Vec<_>>();

        Ok(Self {
            hunks: Arc::new(Mutex::new(hunks)),
            state,
            _mode: mode,
        })
    }

    pub fn accept_selected_hunk(&mut self) -> Result<()> {
        if let Some(selected) = self.selected_hunk() {
            self.state.accept_hunk(selected);
        }

        Ok(())
    }

    pub fn reject_selected_hunk(&mut self) -> Result<()> {
        if let Some(selected) = self.selected_hunk() {
            self.state.reject_hunk(selected);
        }

        Ok(())
    }

    pub fn selected_hunk(&self) -> Option<usize> {
        self.state.selected_hunk()
    }
}

impl<'a> App<'a> {
    fn show_hunk_list(&self, ui: &mut Ui<Message>, frame: &mut Frame) {
        let header = [Column::new(" Hunks ", Constraint::Fill(1))].to_vec();
        let columns = [
            Column::new("", Constraint::Length(2)),
            Column::new("", Constraint::Fill(1)),
            Column::new("", Constraint::Length(15)),
        ]
        .to_vec();

        let hunks = self.hunks.lock().unwrap();
        let hunks = hunks
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                StatefulHunkItem::new(
                    item.clone(),
                    self.state
                        .hunk_states()
                        .get(idx)
                        .cloned()
                        .unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        let mut selected = self.state.selected_hunk();

        let table = ui.headered_table(frame, &mut selected, &hunks, header, columns);
        if table.changed {
            ui.send_message(Message::HunkChanged {
                state: TableState::new(selected),
            })
        }
    }

    fn show_hunk(&self, ui: &mut Ui<Message>, frame: &mut Frame) {
        let hunks = self.hunks.lock().unwrap();

        let selected = self.state.selected_hunk();
        let hunk = selected.and_then(|selected| hunks.get(selected));

        if let Some(hunk) = hunk {
            let empty_text = hunk
                .hunk_text()
                .unwrap_or(Text::raw("Nothing to show.").dark_gray());

            let mut cursor = selected
                .and_then(|selected| self.state.view_state(selected))
                .map(|state| state.cursor)
                .unwrap_or_default();

            ui.composite(layout::container(), 1, |ui| {
                ui.columns(frame, hunk.header(), Some(Borders::Top));

                if let Some(text) = hunk.hunk_text() {
                    let diff = ui.text_view(frame, text, &mut cursor, Some(Borders::BottomSides));
                    if diff.changed {
                        ui.send_message(Message::HunkViewChanged {
                            state: DiffViewState { cursor },
                        })
                    }
                } else {
                    ui.centered_text_view(frame, empty_text, Some(Borders::BottomSides));
                }
            });
        }
    }

    fn show_context_bar(&self, ui: &mut Ui<Message>, frame: &mut Frame) {
        let hunks = &self.hunks.lock().unwrap();

        let id = format!(" {} ", format::cob(&self.state.patch));
        let title = &self.state.title;

        let hunks_total = hunks.len();
        let hunks_accepted = self
            .state
            .hunks
            .1
            .iter()
            .filter(|state| **state == HunkState::Accepted)
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

impl<'a> Show<Message> for App<'a> {
    fn show(&self, ctx: &Context<Message>, frame: &mut Frame) -> Result<(), anyhow::Error> {
        Window::default().show(ctx, |ui| {
            let mut page_focus = self.state.panes.focus();

            match self.state.page {
                AppPage::Main => {
                    ui.layout(layout::page(), Some(0), |ui| {
                        let group = ui.panes(layout::list_item(), &mut page_focus, |ui| {
                            self.show_hunk_list(ui, frame);
                            self.show_hunk(ui, frame);
                        });
                        if group.response.changed {
                            ui.send_message(Message::PanesChanged {
                                state: PanesState::new(self.state.panes.len(), page_focus),
                            });
                        }

                        self.show_context_bar(ui, frame);

                        ui.shortcuts(
                            frame,
                            &[
                                ("c", "comment"),
                                ("a", "accept"),
                                ("r", "reject"),
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
                        if ui.input_global(|key| key == Key::Char('r')) {
                            ui.send_message(Message::Reject);
                        }
                    });
                }
                AppPage::Help => {
                    ui.panes(layout::page(), &mut page_focus, |ui| {
                        ui.composite(layout::container(), 1, |ui| {
                            let header = [Column::new(" Help ", Constraint::Fill(1))].to_vec();
                            let mut cursor = self.state.help.cursor();

                            ui.columns(frame, header, Some(Borders::Top));
                            let help = ui.text_view(
                                frame,
                                help_text().to_string(),
                                &mut cursor,
                                Some(Borders::BottomSides),
                            );
                            if help.changed {
                                ui.send_message(Message::HelpChanged {
                                    state: TextViewState::new(cursor),
                                })
                            }
                        });

                        self.show_context_bar(ui, frame);

                        ui.shortcuts(frame, &[("?", "close"), ("q", "quit")], '∙');
                    });

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
    type Return = Response;

    fn update(&mut self, message: Message) -> Option<Exit<Self::Return>> {
        log::info!("Received message: {:?}", message);

        match message {
            Message::ShowMain => {
                self.state.page = AppPage::Main;
                None
            }
            Message::ShowHelp => {
                self.state.page = AppPage::Help;
                None
            }
            Message::PanesChanged { state } => {
                self.state.panes = state;
                None
            }
            Message::HunkChanged { state } => {
                self.state.update_hunks(state);
                None
            }
            Message::HunkViewChanged { state } => {
                if let Some(selected) = self.state.selected_hunk() {
                    self.state.update_view_state(selected, state);
                }
                None
            }
            Message::HelpChanged { state } => {
                self.state.help = state;
                None
            }
            Message::Comment => Some(Exit {
                value: Some(Response {
                    action: Some(ReviewAction::Comment),
                    state: self.state.clone(),
                }),
            }),
            Message::Accept => {
                match self.accept_selected_hunk() {
                    Ok(()) => log::info!("Accepted selected hunk."),
                    Err(err) => log::info!("An error occured while accepting hunk: {}", err),
                }
                None
            }
            Message::Reject => {
                match self.reject_selected_hunk() {
                    Ok(()) => log::info!("Rejected selected hunk."),
                    Err(err) => log::info!("An error occured while rejecting hunk: {}", err),
                }
                None
            }
            Message::Quit => Some(Exit {
                value: Some(Response {
                    action: None,
                    state: self.state.clone(),
                }),
            }),
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

`Tab`       Focus next pane
`BackTab`   Focus previous pane

`?`         toogle help
`q`         quit / cancel

## Specific keybindings

`c`         comment on hunk
`a`         accept hunk
`d`         discard accepted hunks (reject all)"#
        .into()
}

#[cfg(test)]
mod test {
    use anyhow::*;

    use radicle::patch::Cache;

    use store::Update;

    use super::*;
    use crate::test;

    impl<'a> App<'a> {
        pub fn hunks(&self) -> Vec<HunkItem> {
            self.hunks.lock().unwrap().clone()
        }
    }

    mod fixtures {
        use anyhow::*;

        use radicle::cob::cache::NoCache;
        use radicle::patch::{Cache, PatchMut, Review, ReviewId, Revision, Verdict};
        use radicle::prelude::Signer;
        use radicle::storage::git::cob::DraftStore;
        use radicle::storage::git::Repository;

        use radicle_tui::ui::im::widget::{PanesState, TableState, TextViewState};
        use ratatui::layout::Position;

        use crate::cob::patch;
        use crate::test::setup::NodeWithRepo;

        use super::builder::ReviewBuilder;
        use super::{App, AppPage, AppState, DiffViewState, HunkState, ReviewMode};

        pub fn app<'a>(
            node: &NodeWithRepo,
            patch: PatchMut<Repository, NoCache>,
        ) -> Result<App<'a>> {
            let draft_store = DraftStore::new(&node.repo.repo, *node.signer.public_key());
            let mut drafts = Cache::no_cache(&draft_store)?;
            let mut draft = drafts.get_mut(&patch.id())?;

            let (_, revision) = patch.latest();
            let (_, review) = draft_review(&node, &mut draft, revision)?;

            let hunks = ReviewBuilder::new(&node.repo).hunks(revision)?;

            let state = AppState::new(
                node.repo.id,
                *patch.id(),
                patch.title().to_string(),
                revision.clone(),
                AppPage::Main,
                PanesState::new(2, Some(0)),
                (
                    TableState::new(Some(0)),
                    vec![HunkState::Rejected; hunks.len()],
                ),
                vec![DiffViewState::default(); hunks.len()],
                TextViewState::new(Position::default()),
            );

            App::new(
                ReviewMode::Create,
                node.storage.clone(),
                review.clone(),
                hunks,
                state,
            )
        }

        pub fn draft_review<'a>(
            node: &NodeWithRepo,
            draft: &'a mut PatchMut<DraftStore<Repository>, NoCache>,
            revision: &Revision,
        ) -> Result<(ReviewId, &'a Review)> {
            let id = draft.review(
                revision.id(),
                Some(Verdict::Reject),
                None,
                vec![],
                &node.node.signer,
            )?;

            let (_, review) = patch::find_review(draft, revision, &node.node.signer)
                .ok_or_else(|| anyhow!("Could not find review."))?;

            Ok((id, review))
        }
    }

    #[test]
    fn app_with_single_hunk_can_be_constructed() -> Result<()> {
        let alice = test::fixtures::node_with_repo();
        let branch = test::fixtures::branch_with_main_emptied(&alice);

        let mut patches = Cache::no_cache(&alice.repo.repo).unwrap();
        let patch = test::fixtures::patch(&alice, &branch, &mut patches)?;

        let app = fixtures::app(&alice, patch)?;

        assert_eq!(app.hunks().len(), 1);

        Ok(())
    }

    #[test]
    fn app_with_single_file_multiple_hunks_can_be_constructed() -> Result<()> {
        let alice = test::fixtures::node_with_repo();
        let branch = test::fixtures::branch_with_eof_removed(&alice);

        let mut patches = Cache::no_cache(&alice.repo.repo).unwrap();
        let patch = test::fixtures::patch(&alice, &branch, &mut patches)?;

        let app = fixtures::app(&alice, patch)?;

        assert_eq!(app.hunks().len(), 2);

        Ok(())
    }

    #[test]
    fn first_hunk_is_selected_by_default() -> Result<()> {
        let alice = test::fixtures::node_with_repo();
        let branch = test::fixtures::branch_with_main_emptied(&alice);

        let mut patches = Cache::no_cache(&alice.repo.repo).unwrap();
        let patch = test::fixtures::patch(&alice, &branch, &mut patches)?;

        let app = fixtures::app(&alice, patch)?;

        assert_eq!(app.selected_hunk(), Some(0));

        Ok(())
    }

    #[test]
    fn hunks_are_rejected_by_default() -> Result<()> {
        let alice = test::fixtures::node_with_repo();
        let branch = test::fixtures::branch_with_main_deleted_and_file_added(&alice);

        let mut patches = Cache::no_cache(&alice.repo.repo).unwrap();
        let patch = test::fixtures::patch(&alice, &branch, &mut patches)?;

        let app = fixtures::app(&alice, patch)?;
        let states = &app.state.hunk_states();

        assert_eq!(**states, [HunkState::Rejected, HunkState::Rejected]);

        Ok(())
    }

    #[test]
    fn hunk_can_be_selected() -> Result<()> {
        let alice = test::fixtures::node_with_repo();
        let branch = test::fixtures::branch_with_eof_removed(&alice);

        let mut patches = Cache::no_cache(&alice.repo.repo).unwrap();
        let patch = test::fixtures::patch(&alice, &branch, &mut patches)?;

        let mut app = fixtures::app(&alice, patch)?;
        app.update(Message::HunkChanged {
            state: TableState::new(Some(1)),
        });

        assert_eq!(app.selected_hunk(), Some(1));

        Ok(())
    }

    #[test]
    fn single_file_single_hunk_can_be_accepted() -> Result<()> {
        let alice = test::fixtures::node_with_repo();
        let branch = test::fixtures::branch_with_main_emptied(&alice);

        let mut patches = Cache::no_cache(&alice.repo.repo).unwrap();
        let patch = test::fixtures::patch(&alice, &branch, &mut patches)?;

        let mut app = fixtures::app(&alice, patch)?;
        app.update(Message::Accept);

        let state = &app.state.hunk_states().get(0).unwrap();

        assert_eq!(**state, HunkState::Accepted);

        Ok(())
    }

    #[test]
    fn single_file_multiple_hunks_only_first_can_be_accepted() -> Result<()> {
        let alice = test::fixtures::node_with_repo();
        let branch = test::fixtures::branch_with_main_changed(&alice);

        let mut patches = Cache::no_cache(&alice.repo.repo).unwrap();
        let patch = test::fixtures::patch(&alice, &branch, &mut patches)?;

        let mut app = fixtures::app(&alice, patch)?;
        app.update(Message::Accept);

        let states = &app.state.hunk_states();

        assert_eq!(**states, [HunkState::Accepted, HunkState::Rejected]);

        Ok(())
    }

    #[test]
    fn single_file_multiple_hunks_only_last_can_be_accepted() -> Result<()> {
        let alice = test::fixtures::node_with_repo();
        let branch = test::fixtures::branch_with_main_changed(&alice);

        let mut patches = Cache::no_cache(&alice.repo.repo).unwrap();
        let patch = test::fixtures::patch(&alice, &branch, &mut patches)?;

        let mut app = fixtures::app(&alice, patch)?;

        app.update(Message::HunkChanged {
            state: TableState::new(Some(1)),
        });
        app.update(Message::Accept);

        let states = &app.state.hunk_states();

        assert_eq!(**states, [HunkState::Rejected, HunkState::Accepted]);

        Ok(())
    }

    #[test]
    fn multiple_files_single_hunk_can_be_accepted() -> Result<()> {
        let alice = test::fixtures::node_with_repo();
        let branch = test::fixtures::branch_with_main_deleted_and_file_added(&alice);

        let mut patches = Cache::no_cache(&alice.repo.repo).unwrap();
        let patch = test::fixtures::patch(&alice, &branch, &mut patches)?;

        let mut app = fixtures::app(&alice, patch)?;
        app.update(Message::Accept);

        app.update(Message::HunkChanged {
            state: TableState::new(Some(1)),
        });
        app.update(Message::Accept);

        let states = &app.state.hunk_states();

        assert_eq!(**states, [HunkState::Accepted, HunkState::Accepted]);

        Ok(())
    }
}
