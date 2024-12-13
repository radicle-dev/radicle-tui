#[path = "review/builder.rs"]
pub mod builder;

use std::fmt::Debug;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;

use radicle::storage::git::Repository;
use termion::event::Key;

use ratatui::layout::Constraint;
use ratatui::layout::Position;
use ratatui::style::Stylize;
use ratatui::text::Text;
use ratatui::{Frame, Viewport};

use radicle::crypto::Signer;
use radicle::identity::RepoId;
use radicle::patch::PatchId;
use radicle::patch::Review;
use radicle::patch::Revision;
use radicle::storage::ReadStorage;
use radicle::storage::WriteRepository;
use radicle::Storage;

use radicle_tui as tui;

use tui::store;
use tui::ui::im::widget::PanesState;
use tui::ui::im::widget::{TableState, TextViewState, Window};
use tui::ui::im::Ui;
use tui::ui::im::{Borders, Context, Show};
use tui::ui::span;
use tui::ui::Column;
use tui::{Channel, Exit};

use crate::git::HunkState;
use crate::git::StatefulHunkDiff;
use crate::tui_patch::review::builder::DiffUtil;
use crate::ui::format;
use crate::ui::items::HunkItem;
use crate::ui::layout;

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
    pub storage: Storage,
    pub rid: RepoId,
    pub signer: Box<dyn Signer>,
    pub patch: PatchId,
    pub title: String,
    pub revision: Revision,
    pub review: Review,
    pub hunks: ReviewQueue,
}

impl Tui {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        storage: Storage,
        rid: RepoId,
        signer: Box<dyn Signer>,
        patch: PatchId,
        title: String,
        revision: Revision,
        review: Review,
        hunks: ReviewQueue,
    ) -> Self {
        Self {
            storage,
            rid,
            signer,
            patch,
            title,
            revision,
            review,
            hunks,
        }
    }

    pub async fn run(self) -> Result<Option<Selection>> {
        let viewport = Viewport::Fullscreen;

        let channel = Channel::default();
        let state = App::new(
            self.storage,
            self.rid,
            self.signer,
            self.patch,
            self.title,
            self.revision,
            self.review,
            self.hunks,
        )?;

        tui::im(state, viewport, channel).await
    }
}

#[derive(Clone, Debug)]
pub enum Message<'a> {
    ShowMain,
    PanesChanged { state: PanesState },
    HunkChanged { state: TableState },
    HunkViewChanged { state: HunkItemState },
    ShowHelp,
    HelpChanged { state: TextViewState<'a> },
    Comment,
    Accept,
    Discard,
    Quit,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum AppPage {
    Main,
    Help,
}

#[derive(Clone, Debug)]
pub struct HunkItemState {
    cursor: Position,
}

pub type HunkItems<'a> = Vec<HunkItem<'a>>;
pub type HunkItemStates = Vec<HunkItemState>;

#[derive(Clone)]
pub struct App<'a> {
    /// The nodes' storage.
    storage: Storage,
    /// The repository to operate on.
    rid: RepoId,
    /// Signer of all writes to the storage or repo.
    signer: Arc<Mutex<Box<dyn Signer>>>,
    /// Patch this review belongs to.
    patch: PatchId,
    /// Title of the patch this patch this review belongs to.
    title: String,
    /// Revision this review belongs to.
    revision: Revision,
    /// All hunks, their states and the lists' table widget state.
    hunks: Arc<Mutex<(HunkItems<'a>, HunkItemStates, TableState)>>,
    /// Current app page.
    page: AppPage,
    /// State of panes widget on the main page.
    group: PanesState,
    /// State of text view widget on the help page.
    help: TextViewState<'a>,
}

impl<'a> TryFrom<Tui> for App<'a> {
    type Error = anyhow::Error;

    fn try_from(tui: Tui) -> Result<Self, Self::Error> {
        App::new(
            tui.storage,
            tui.rid,
            tui.signer,
            tui.patch,
            tui.title,
            tui.revision,
            tui.review,
            tui.hunks,
        )
    }
}

impl<'a> App<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        storage: Storage,
        rid: RepoId,
        signer: Box<dyn Signer>,
        patch: PatchId,
        title: String,
        revision: Revision,
        review: Review,
        hunks: ReviewQueue,
    ) -> Result<Self, anyhow::Error> {
        let repo = storage.repository(rid)?;
        let states = hunks
            .iter()
            .map(|_| HunkItemState {
                cursor: Position::new(0, 0),
            })
            .collect::<Vec<_>>();
        let hunks = hunks
            .iter()
            .map(|(_, item)| HunkItem::from((&repo, &review, StatefulHunkDiff::from(item))))
            .collect::<Vec<_>>();

        let mut app = App {
            storage,
            signer: Arc::new(Mutex::new(signer)),
            rid,
            patch,
            title,
            revision,
            hunks: Arc::new(Mutex::new((hunks, states, TableState::new(Some(0))))),
            page: AppPage::Main,
            group: PanesState::new(2, Some(0)),
            help: TextViewState::new(help_text(), Position::default()),
        };

        app.reload_states()?;

        Ok(app)
    }

    #[allow(clippy::borrowed_box)]
    pub fn accept_current_hunk(&self) -> Result<()> {
        let repo = self.storage.repository(self.rid).unwrap();
        let signer: &Box<dyn Signer> = &self.signer.lock().unwrap();

        if let Some(selected) = self.selected_hunk_idx() {
            let mut brain = Brain::load_or_new(self.patch, &self.revision, repo.raw(), signer)?;
            let hunks = self.hunks.lock().unwrap().0.clone();

            if let Some(hunk) = hunks.get(selected) {
                let mut file: Option<FileReviewBuilder> = None;
                let file = match file.as_mut() {
                    Some(fr) => fr.set_item(hunk.inner.hunk()),
                    None => file.insert(FileReviewBuilder::new(hunk.inner.hunk())),
                };

                let diff = file.item_diff(hunk.inner.hunk())?;
                brain.accept(diff, repo.raw())?;
            }
        }

        Ok(())
    }

    #[allow(clippy::borrowed_box)]
    pub fn discard_accepted_hunks(&self) -> Result<()> {
        let repo = self.repo()?;
        let signer: &Box<dyn Signer> = &self.signer.lock().unwrap();

        let mut brain = Brain::load_or_new(self.patch, &self.revision, repo.raw(), signer)?;
        brain.discard_accepted(repo.raw())?;

        Ok(())
    }

    #[allow(clippy::borrowed_box)]
    pub fn reload_states(&mut self) -> anyhow::Result<()> {
        let repo = self.repo()?;
        let signer: &Box<dyn Signer> = &self.signer.lock().unwrap();

        let brain = Brain::load_or_new(self.patch, &self.revision, repo.raw(), signer)?;
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

        let mut queue = self.hunks.lock().unwrap();
        for (idx, new_state) in states.iter().enumerate() {
            if let Some(hunk) = queue.0.get_mut(idx) {
                *hunk.inner.state_mut() = new_state.clone();
            }
        }

        Ok(())
    }

    pub fn selected_hunk_idx(&self) -> Option<usize> {
        self.hunks.lock().unwrap().2.selected()
    }

    pub fn repo(&self) -> Result<Repository> {
        Ok(self.storage.repository(self.rid)?)
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

        let hunks = self.hunks.lock().unwrap();
        let mut selected = hunks.2.selected();

        let table = ui.headered_table(frame, &mut selected, &hunks.0, header, columns);
        if table.changed {
            ui.send_message(Message::HunkChanged {
                state: TableState::new(selected),
            })
        }
    }

    fn show_hunk(&self, ui: &mut Ui<Message<'a>>, frame: &mut Frame) {
        let hunks = self.hunks.lock().unwrap();

        let selected = hunks.2.selected();
        let hunk = selected.and_then(|selected| hunks.0.get(selected));

        if let Some(hunk) = hunk {
            let empty_text = hunk
                .hunk_text()
                .unwrap_or(Text::raw("Nothing to show.").dark_gray());

            let mut cursor = selected
                .and_then(|selected| hunks.1.get(selected))
                .map(|state| state.cursor)
                .unwrap_or_default();

            ui.composite(layout::container(), 1, |ui| {
                ui.columns(frame, hunk.header(), Some(Borders::Top));

                if let Some(text) = hunk.hunk_text() {
                    let diff = ui.text_view(frame, text, &mut cursor, Some(Borders::BottomSides));
                    if diff.changed {
                        ui.send_message(Message::HunkViewChanged {
                            state: HunkItemState { cursor },
                        })
                    }
                } else {
                    ui.centered_text_view(frame, empty_text, Some(Borders::BottomSides));
                }
            });
        }
    }

    fn show_context_bar(&self, ui: &mut Ui<Message<'a>>, frame: &mut Frame) {
        let hunks = &self.hunks.lock().unwrap().0;

        let id = format!(" {} ", format::cob(&self.patch));
        let title = &self.title;

        let hunks_total = hunks.len();
        let hunks_accepted = hunks
            .iter()
            .filter(|hunk| *hunk.inner.state() == HunkState::Accepted)
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
            let mut page_focus = self.group.focus();

            match self.page {
                AppPage::Main => {
                    ui.layout(layout::page(), Some(0), |ui| {
                        let group = ui.panes(layout::list_item(), &mut page_focus, |ui| {
                            self.show_hunk_list(ui, frame);
                            self.show_hunk(ui, frame);
                        });
                        if group.response.changed {
                            ui.send_message(Message::PanesChanged {
                                state: PanesState::new(self.group.len(), page_focus),
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
                    });
                }
                AppPage::Help => {
                    ui.panes(layout::page(), &mut page_focus, |ui| {
                        ui.composite(layout::container(), 1, |ui| {
                            let header = [Column::new(" Help ", Constraint::Fill(1))].to_vec();
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
                                    state: TextViewState::new(self.help.text().clone(), cursor),
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

impl<'a> store::Update<Message<'a>> for App<'a> {
    type Return = Selection;

    fn update(&mut self, message: Message<'a>) -> Option<Exit<Self::Return>> {
        log::info!("Received message: {:?}", message);

        match message {
            Message::ShowMain => {
                self.page = AppPage::Main;
                None
            }
            Message::ShowHelp => {
                self.page = AppPage::Help;
                None
            }
            Message::PanesChanged { state } => {
                self.group = state;
                None
            }
            Message::HunkChanged { state } => {
                let mut hunks = self.hunks.lock().unwrap();
                hunks.2 = state;
                None
            }
            Message::HunkViewChanged { state } => {
                let mut hunks = self.hunks.lock().unwrap();
                if let Some(selected) = hunks.2.selected() {
                    if let Some(item_state) = hunks.1.get_mut(selected) {
                        *item_state = state;
                    }
                }
                None
            }
            Message::HelpChanged { state } => {
                self.help = state;
                None
            }
            Message::Comment => {
                let hunks = self.hunks.lock().unwrap();
                Some(Exit {
                    value: Some(Selection {
                        action: ReviewAction::Comment,
                        hunk: hunks.2.selected(),
                        args: None,
                    }),
                })
            }
            Message::Accept => {
                match self.accept_current_hunk() {
                    Ok(()) => log::info!("Accepted hunk."),
                    Err(err) => log::info!("An error occured while accepting hunk: {}", err),
                }
                let _ = self.reload_states();
                None
            }
            Message::Discard => {
                match self.discard_accepted_hunks() {
                    Ok(()) => log::info!("Discarded all hunks."),
                    Err(err) => log::info!("An error occured while discarding hunks: {}", err),
                }
                let _ = self.reload_states();
                None
            }
            Message::Quit => Some(Exit { value: None }),
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
            self.hunks.lock().unwrap().0.clone()
        }
    }

    mod fixtures {
        use anyhow::*;

        use radicle::cob::cache::NoCache;
        use radicle::patch::{Cache, PatchMut, Review, ReviewId, Revision, Verdict};
        use radicle::prelude::Signer;
        use radicle::storage::git::cob::DraftStore;
        use radicle::storage::git::Repository;
        use radicle::storage::WriteRepository;
        use radicle::test::setup::NodeWithRepo;

        use crate::cob::patch;

        use super::builder::{Brain, ReviewBuilder};
        use super::App;

        pub fn app<'a>(
            node: &NodeWithRepo,
            patch: PatchMut<Repository, NoCache>,
        ) -> Result<App<'a>> {
            let draft_store = DraftStore::new(&node.repo.repo, *node.signer.public_key());
            let mut drafts = Cache::no_cache(&draft_store)?;
            let mut draft = drafts.get_mut(&patch.id())?;

            let (_, revision) = patch.latest();
            let (_, review) = draft_review(&node, &mut draft, revision)?;

            let brain = Brain::load_or_new(*patch.id(), revision, node.repo.raw(), &node.signer)?;
            let hunks = ReviewBuilder::new(&node.repo).hunks(&brain, revision)?;

            App::new(
                node.storage.clone(),
                node.repo.id,
                Box::new(node.signer.clone()),
                *patch.id(),
                patch.title().to_string(),
                revision.clone(),
                review.clone(),
                hunks,
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
        let branch = test::fixtures::branch_with_line_added(&alice);

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
        let branch = test::fixtures::branch_with_line_added(&alice);

        let mut patches = Cache::no_cache(&alice.repo.repo).unwrap();
        let patch = test::fixtures::patch(&alice, &branch, &mut patches)?;

        let app = fixtures::app(&alice, patch)?;

        assert_eq!(app.selected_hunk_idx(), Some(0));

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

        assert_eq!(app.selected_hunk_idx(), Some(1));

        Ok(())
    }

    #[test]
    fn single_file_single_hunk_can_be_accepted() -> Result<()> {
        let alice = test::fixtures::node_with_repo();
        let branch = test::fixtures::branch_with_line_added(&alice);

        let mut patches = Cache::no_cache(&alice.repo.repo).unwrap();
        let patch = test::fixtures::patch(&alice, &branch, &mut patches)?;

        let mut app = fixtures::app(&alice, patch)?;
        app.update(Message::Accept);

        let hunks = app.hunks();
        let state = &hunks.get(0).unwrap().inner.state();

        assert_eq!(**state, HunkState::Accepted);

        Ok(())
    }

    #[test]
    fn multiple_files_single_hunk_can_be_accepted() -> Result<()> {
        let alice = test::fixtures::node_with_repo();
        let branch = test::fixtures::branch_with_files_added(&alice);

        let mut patches = Cache::no_cache(&alice.repo.repo).unwrap();
        let patch = test::fixtures::patch(&alice, &branch, &mut patches)?;

        let mut app = fixtures::app(&alice, patch)?;
        app.update(Message::Accept);

        app.update(Message::HunkChanged {
            state: TableState::new(Some(1)),
        });
        app.update(Message::Accept);

        let hunks = app.hunks();
        let states = [
            hunks.get(0).unwrap().inner.state(),
            hunks.get(1).unwrap().inner.state(),
        ];

        assert_eq!(states, [&HunkState::Accepted, &HunkState::Accepted]);

        Ok(())
    }
}
