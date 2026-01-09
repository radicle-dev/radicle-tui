use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::vec;

use anyhow::Result;

use serde::Serialize;

use radicle::node::notifications::NotificationId;
use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::*;
use ratatui::text::Span;
use ratatui::{Frame, Viewport};

use radicle::identity::Project;
use radicle::prelude::RepoId;
use radicle::storage::ReadStorage;
use radicle::Profile;

use radicle_tui as tui;

use tui::event::Key;
use tui::store;
use tui::task::{Process, Task};
use tui::ui;
use tui::ui::layout::Spacing;
use tui::ui::widget::{
    Borders, Column, ContainerState, TableState, TextEditState, TextViewState, Window,
};
use tui::ui::{BufferedValue, Show, Ui};
use tui::{Channel, Exit};

use crate::ui::items::filter::Filter;
use crate::ui::items::notification::filter::{NotificationFilter, SortBy};
use crate::ui::items::notification::{Notification, NotificationKind};

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub enum RepositoryMode {
    #[default]
    Contextual,
    All,
    ByRepo((RepoId, Option<String>)),
}

/// The selected issue operation returned by the operation
/// selection widget.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum InboxOperation {
    Show { id: NotificationId, search: String },
    Clear { id: NotificationId, search: String },
}

type Selection = tui::Selection<InboxOperation>;

const HELP: &str = r#"# Generic keybindings

`↑,k`:      move cursor one line up
`↓,j:       move cursor one line down
`PageUp`:   move cursor one page up
`PageDown`: move cursor one page down
`Home`:     move cursor to the first line
`End`:      move cursor to the last line
`Esc`:      Cancel
`q`:        Quit

# Specific keybindings

`enter`:    Show notification
`r`:        Reload notifications
`c`:        Clear notification
`/`:        Search
`?`:        Show help

# Searching

Examples:   state=unseen kind=cob bugfix
            kind=(cob:xyz.radicle.issue or cob:xyz.radicle.issue)
            state=unseen author=(did:key:... or did:key:...)"#;

#[derive(Clone, Debug)]
pub struct Context {
    pub profile: Profile,
    pub project: Project,
    pub rid: RepoId,
    pub mode: RepositoryMode,
    pub sort_by: SortBy,
    pub _notif_id: Option<NotificationId>,
    pub search: Option<String>,
}

pub struct Tui {
    context: Context,
}

impl Tui {
    pub fn new(context: Context) -> Self {
        Self { context }
    }

    pub async fn run(&self) -> Result<Option<Selection>> {
        let viewport = Viewport::Inline(20);
        let channel = Channel::default();
        let state = App::try_from(&self.context)?;

        tui::im(
            state,
            viewport,
            channel,
            vec![Loader::new(self.context.clone())],
        )
        .await
    }
}

#[derive(Clone, Debug)]
pub enum Change {
    Page {
        page: Page,
    },
    MainGroup {
        state: ContainerState,
    },
    Patches {
        state: TableState,
    },
    Search {
        search: BufferedValue<TextEditState>,
    },
    Help {
        state: TextViewState,
    },
}

#[derive(Clone, Debug)]
pub enum Message {
    Initialize,
    Changed(Change),
    ShowSearch,
    HideSearch { apply: bool },
    Reload,
    Loaded(Vec<Notification>),
    Exit { operation: Option<InboxOperation> },
    Quit,
}

#[derive(Clone, Debug)]
pub enum Page {
    Main,
    Help,
}

#[derive(Clone, Debug)]
pub struct AppState {
    page: Page,
    main_group: ContainerState,
    patches: TableState,
    search: BufferedValue<TextEditState>,
    show_search: bool,
    help: TextViewState,
    filter: NotificationFilter,
    loading: bool,
    initialized: bool,
}

#[derive(Clone, Debug)]
pub struct App {
    context: Arc<Mutex<Context>>,
    notifications: Arc<Mutex<Vec<Notification>>>,
    state: AppState,
}

impl TryFrom<&Context> for App {
    type Error = anyhow::Error;

    fn try_from(context: &Context) -> Result<Self, Self::Error> {
        let search = context.search.as_ref().map(|s| s.trim().to_string());
        let (search, filter) = match search {
            Some(search) => (
                search.clone(),
                NotificationFilter::from_str(search.trim()).unwrap_or(NotificationFilter::Invalid),
            ),
            None => {
                let filter = NotificationFilter::default();
                (filter.to_string().trim().to_string(), filter)
            }
        };

        Ok(App {
            context: Arc::new(Mutex::new(context.clone())),
            notifications: Arc::new(Mutex::new(vec![])),
            state: AppState {
                page: Page::Main,
                main_group: ContainerState::new(3, Some(0)),
                patches: TableState::new(Some(0)),
                search: BufferedValue::new(TextEditState {
                    text: search.to_string(),
                    cursor: search.chars().count(),
                }),
                show_search: false,
                help: TextViewState::new(Position::default()),
                filter,
                loading: false,
                initialized: false,
            },
        })
    }
}

impl store::Update<Message> for App {
    type Return = Selection;

    fn update(&mut self, message: Message) -> Option<tui::Exit<Selection>> {
        match message {
            Message::Initialize => {
                self.state.loading = true;
                self.state.initialized = true;
                None
            }
            Message::Quit => Some(Exit { value: None }),
            Message::Exit { operation } => Some(Exit {
                value: Some(Selection {
                    operation,
                    args: vec![],
                }),
            }),
            Message::ShowSearch => {
                self.state.main_group = ContainerState::new(3, None);
                self.state.show_search = true;
                None
            }
            Message::HideSearch { apply } => {
                self.state.main_group = ContainerState::new(3, Some(0));
                self.state.show_search = false;

                if apply {
                    self.state.search.apply();
                } else {
                    self.state.search.reset();
                }

                self.state.filter = NotificationFilter::from_str(&self.state.search.read().text)
                    .unwrap_or(NotificationFilter::Invalid);

                None
            }
            Message::Reload => {
                self.state.loading = true;
                None
            }
            Message::Loaded(notifications) => {
                self.apply_notifications(notifications);
                self.apply_sorting();
                self.state.loading = false;
                None
            }
            Message::Changed(changed) => match changed {
                Change::Page { page } => {
                    self.state.page = page;
                    None
                }
                Change::MainGroup { state } => {
                    self.state.main_group = state;
                    None
                }
                Change::Patches { state } => {
                    self.state.patches = state;
                    None
                }
                Change::Search { search } => {
                    self.state.search = search;
                    self.state.filter =
                        NotificationFilter::from_str(&self.state.search.read().text)
                            .unwrap_or(NotificationFilter::Invalid);
                    self.state.patches.select_first();
                    None
                }
                Change::Help { state } => {
                    self.state.help = state;
                    None
                }
            },
        }
    }
}

impl Show<Message> for App {
    fn show(&self, ctx: &ui::Context<Message>, frame: &mut Frame) -> Result<()> {
        Window::default().show(ctx, |ui| {
            // Initialize
            if !self.state.initialized {
                ui.send_message(Message::Initialize);
            }

            match self.state.page {
                Page::Main => {
                    let show_search = self.state.show_search;
                    let mut page_focus = if show_search { Some(1) } else { Some(0) };

                    ui.container(
                        Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]),
                        &mut page_focus,
                        |ui| {
                            let mut group_focus = self.state.main_group.focus();

                            let group = ui.container(
                                ui::Layout::Expandable3 { left_only: true },
                                &mut group_focus,
                                |ui| {
                                    self.show_browser(frame, ui);
                                },
                            );
                            if group.response.changed {
                                ui.send_message(Message::Changed(Change::MainGroup {
                                    state: ContainerState::new(3, group_focus),
                                }));
                            }

                            if show_search {
                                self.show_browser_search(frame, ui);
                            } else if let Some(0) = group_focus {
                                self.show_browser_footer(frame, ui);
                            }
                        },
                    );

                    if !show_search && ui.has_input(|key| key == Key::Char('?')) {
                        ui.send_message(Message::Changed(Change::Page { page: Page::Help }));
                    }
                }

                Page::Help => {
                    let layout = Layout::vertical([
                        Constraint::Length(3),
                        Constraint::Fill(1),
                        Constraint::Length(1),
                        Constraint::Length(1),
                    ]);

                    ui.container(layout, &mut Some(1), |ui| {
                        self.show_help_text(frame, ui);
                        self.show_help_context(frame, ui);

                        ui.shortcuts(frame, &[("?", "close")], '∙', Alignment::Left);
                    });

                    if ui.has_input(|key| key == Key::Char('?')) {
                        ui.send_message(Message::Changed(Change::Page { page: Page::Main }));
                    }
                }
            }
            if ui.has_input(|key| key == Key::Char('q')) {
                ui.send_message(Message::Quit);
            }
            if ui.has_input(|key| key == Key::Ctrl('c')) {
                ui.send_message(Message::Quit);
            }
        });

        Ok(())
    }
}

impl App {
    pub fn show_browser(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        let context = self.context.lock().unwrap();
        let notifs = self.notifications.lock().unwrap();
        let notifs = notifs
            .iter()
            .filter(|notif| self.state.filter.matches(notif))
            .cloned()
            .collect::<Vec<_>>();
        let mut selected = self.state.patches.selected();

        let header = [
            Column::new(Span::raw(" ● ").bold(), Constraint::Length(3)),
            Column::new(Span::raw("ID").bold(), Constraint::Length(8)).hide_medium(),
            Column::new(Span::raw("Summary").bold(), Constraint::Fill(1)),
            Column::new(Span::raw("Repository").bold(), Constraint::Length(16))
                .skip(context.mode != RepositoryMode::All),
            Column::new(Span::raw("OID").bold(), Constraint::Length(8)).hide_medium(),
            Column::new(Span::raw("Kind").bold(), Constraint::Length(20)).hide_small(),
            Column::new(Span::raw("Change").bold(), Constraint::Length(8)).hide_small(),
            Column::new(Span::raw("Author").bold(), Constraint::Length(16)).hide_medium(),
            Column::new(Span::raw("Updated").bold(), Constraint::Length(16)),
        ];

        ui.layout(
            Layout::vertical([Constraint::Length(3), Constraint::Min(1)]),
            Some(1),
            |ui| {
                ui.column_bar(frame, header.to_vec(), Spacing::from(1), Some(Borders::Top));

                let table = ui.table(
                    frame,
                    &mut selected,
                    &notifs,
                    header.to_vec(),
                    Some("".into()),
                    Spacing::from(1),
                    Some(Borders::BottomSides),
                );
                if table.changed {
                    ui.send_message(Message::Changed(Change::Patches {
                        state: TableState::new(selected),
                    }));
                }

                if self.state.loading {
                    self.show_loading_popup(frame, ui);
                }

                // TODO(erikli): Should only work if table has focus
                if ui.has_input(|key| key == Key::Char('/')) {
                    ui.send_message(Message::ShowSearch);
                }
            },
        );

        if ui.has_input(|key| key == Key::Char('r')) {
            ui.send_message(Message::Reload);
        }

        if let Some(notification) = selected.and_then(|s| notifs.get(s)) {
            if ui.has_input(|key| key == Key::Enter) {
                ui.send_message(Message::Exit {
                    operation: Some(InboxOperation::Show {
                        id: notification.id,
                        search: self.state.search.read().text,
                    }),
                });
            }
            if ui.has_input(|key| key == Key::Char('c')) {
                ui.send_message(Message::Exit {
                    operation: Some(InboxOperation::Clear {
                        id: notification.id,
                        search: self.state.search.read().text,
                    }),
                });
            }
        }
    }

    fn show_browser_footer(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        ui.layout(Layout::vertical([3, 1]), None, |ui| {
            self.show_browser_context(frame, ui);
            self.show_browser_shortcuts(frame, ui);
        });
    }

    pub fn show_browser_search(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        let (mut search_text, mut search_cursor) = (
            self.state.search.clone().read().text,
            self.state.search.clone().read().cursor,
        );
        let mut search = self.state.search.clone();

        let text_edit = ui.text_edit_singleline(
            frame,
            &mut search_text,
            &mut search_cursor,
            Some("Search".to_string()),
            Some(Borders::Spacer { top: 0, left: 0 }),
        );

        if text_edit.changed {
            search.write(TextEditState {
                text: search_text,
                cursor: search_cursor,
            });
            ui.send_message(Message::Changed(Change::Search { search }));
        }

        if ui.has_input(|key| key == Key::Esc) {
            ui.send_message(Message::HideSearch { apply: false });
        }
        if ui.has_input(|key| key == Key::Enter) {
            ui.send_message(Message::HideSearch { apply: true });
        }
    }

    fn show_browser_context(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        let context = {
            let notifs = self.notifications.lock().unwrap();
            let search = self.state.search.read().text;
            let total_count = notifs.len();
            let filtered_count = notifs
                .iter()
                .filter(|patch| self.state.filter.matches(patch))
                .collect::<Vec<_>>()
                .len();

            let filtered_counts = format!(" {filtered_count}/{total_count} ");
            let seen_counts = notifs
                .iter()
                .fold((0, 0), |counts, notif| match notif.seen {
                    true => (counts.0 + 1, counts.1),
                    false => (counts.0, counts.1 + 1),
                });

            if self.state.filter.is_default() {
                let seen = format!(" {} ", seen_counts.0);
                let unseen = format!(" {} ", seen_counts.1);
                [
                    Column::new(
                        Span::raw(" Search ".to_string()).cyan().dim().reversed(),
                        Constraint::Length(8),
                    ),
                    Column::new(
                        Span::raw(format!(" {search} "))
                            .into_left_aligned_line()
                            .style(ui.theme().bar_on_black_style)
                            .cyan()
                            .dim(),
                        Constraint::Fill(1),
                    ),
                    Column::new(
                        Span::raw("●")
                            .style(ui.theme().bar_on_black_style)
                            .gray()
                            .dim()
                            .bold(),
                        Constraint::Length(1),
                    ),
                    Column::new(
                        Span::raw(seen.clone())
                            .style(ui.theme().bar_on_black_style)
                            .dim(),
                        Constraint::Length(seen.chars().count() as u16),
                    ),
                    Column::new(
                        Span::raw("●")
                            .style(ui.theme().bar_on_black_style)
                            .cyan()
                            .dim()
                            .bold(),
                        Constraint::Length(1),
                    ),
                    Column::new(
                        Span::raw(unseen.clone())
                            .style(ui.theme().bar_on_black_style)
                            .dim(),
                        Constraint::Length(unseen.chars().count() as u16),
                    ),
                    Column::new(
                        Span::raw(filtered_counts.clone())
                            .into_right_aligned_line()
                            .cyan()
                            .dim()
                            .reversed(),
                        Constraint::Length(filtered_counts.chars().count() as u16),
                    ),
                ]
                .to_vec()
            } else {
                [
                    Column::new(
                        Span::raw(" Search ".to_string()).cyan().dim().reversed(),
                        Constraint::Length(8),
                    ),
                    Column::new(
                        Span::raw(format!(" {search} "))
                            .into_left_aligned_line()
                            .style(ui.theme().bar_on_black_style)
                            .cyan(),
                        Constraint::Fill(1),
                    ),
                    Column::new(
                        Span::raw(filtered_counts.clone())
                            .into_right_aligned_line()
                            .cyan()
                            .dim()
                            .reversed(),
                        Constraint::Length(filtered_counts.chars().count() as u16),
                    ),
                ]
                .to_vec()
            }
        };

        ui.column_bar(frame, context, Spacing::from(0), Some(Borders::None));
    }

    pub fn show_browser_shortcuts(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        ui.shortcuts(
            frame,
            &[
                ("enter", "show"),
                ("r", "reload"),
                ("c", "clear"),
                ("/", "search"),
                ("?", "help"),
            ],
            '∙',
            Alignment::Left,
        );
    }

    fn show_loading_popup(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        ui.popup(Layout::vertical([Constraint::Min(1)]), |ui| {
            ui.layout(
                Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).margin(1),
                None,
                |ui| {
                    ui.label(frame, "");
                    ui.layout(
                        Layout::horizontal([Constraint::Min(1), Constraint::Length(11)]),
                        None,
                        |ui| {
                            ui.label(frame, "");
                            ui.column_bar(
                                frame,
                                [Column::new(
                                    Span::raw(" Loading ").magenta().slow_blink(),
                                    Constraint::Fill(1),
                                )]
                                .to_vec(),
                                Spacing::from(0),
                                Some(Borders::All),
                            );
                        },
                    );
                },
            );
            ui.centered_text_view(frame, "Loading".slow_blink().yellow(), None);
        });
    }

    fn show_help_text(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        ui.column_bar(
            frame,
            [Column::new(Span::raw(" Help ").bold(), Constraint::Fill(1))].to_vec(),
            Spacing::from(0),
            Some(Borders::Top),
        );

        let mut cursor = self.state.help.cursor();
        let text_view = ui.text_view(
            frame,
            HELP.to_string(),
            &mut cursor,
            Some(Borders::BottomSides),
        );
        if text_view.changed {
            ui.send_message(Message::Changed(Change::Help {
                state: TextViewState::new(cursor),
            }))
        }
    }

    fn show_help_context(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        ui.column_bar(
            frame,
            [
                Column::new(
                    Span::raw(" ".to_string())
                        .into_left_aligned_line()
                        .style(ui.theme().bar_on_black_style),
                    Constraint::Fill(1),
                ),
                Column::new(
                    Span::raw(" ")
                        .into_right_aligned_line()
                        .cyan()
                        .dim()
                        .reversed(),
                    Constraint::Length(6),
                ),
            ]
            .to_vec(),
            Spacing::from(0),
            Some(Borders::None),
        );
    }
}

impl App {
    fn apply_notifications(&mut self, notifications: Vec<Notification>) {
        let mut items = self.notifications.lock().unwrap();
        *items = notifications;
    }

    fn apply_sorting(&mut self) {
        let mut items = self.notifications.lock().unwrap();
        let context = self.context.lock().unwrap();
        // Apply sorting
        match context.sort_by.field {
            "timestamp" => items.sort_by(|a, b| a.timestamp.cmp(&b.timestamp)),
            "id" => items.sort_by(|a, b| a.id.cmp(&b.id)),
            _ => {}
        }
        if context.sort_by.reverse {
            items.reverse();
        }

        // Set project name
        let mode = match context.mode {
            RepositoryMode::ByRepo((rid, _)) => {
                let name = context.project.name().to_string();
                RepositoryMode::ByRepo((rid, Some(name)))
            }
            _ => context.mode.clone(),
        };

        // Sort by project if all notifications are shown
        if let RepositoryMode::All = mode {
            items.sort_by(|a, b| a.project.cmp(&b.project));
        }
    }
}

#[derive(Clone, Debug)]
pub struct Loader {
    context: Context,
}

impl Loader {
    fn new(context: Context) -> Self {
        Self { context }
    }
}

#[derive(Debug)]
pub struct NotificationLoader {
    context: Context,
}

impl NotificationLoader {
    fn new(context: Context) -> Self {
        NotificationLoader { context }
    }
}

impl Task for NotificationLoader {
    type Return = Message;

    fn run(&self) -> anyhow::Result<Vec<Self::Return>> {
        let profile = self.context.profile.clone();
        let notifs = profile.notifications_mut()?;

        let notifications = match self.context.mode {
            RepositoryMode::All => {
                // Store all repos the notifs arised from, such that
                // they can be referenced when loading issues and patches
                let repos = notifs
                    .all()?
                    .filter_map(|notif| notif.ok())
                    .filter_map(|notif| {
                        profile
                            .storage
                            .repository(notif.repo)
                            .ok()
                            .map(|repo| (notif.repo, repo))
                    })
                    .collect::<HashMap<_, _>>();

                // Only retrieve issues and patches once per repository
                let (mut issues, mut patches) = (HashMap::new(), HashMap::new());
                notifs
                    .all()?
                    .filter_map(|notif| notif.ok())
                    .map(|notif| match repos.get(&notif.repo) {
                        Some(repo) => {
                            let project = repo.project()?;
                            let (issues, patches) = {
                                (
                                    match issues.entry(repo.id) {
                                        Entry::Occupied(e) => e.into_mut(),
                                        Entry::Vacant(e) => e.insert(profile.issues(repo)?),
                                    },
                                    match patches.entry(repo.id) {
                                        Entry::Occupied(e) => e.into_mut(),
                                        Entry::Vacant(e) => e.insert(profile.patches(repo)?),
                                    },
                                )
                            };

                            match NotificationKind::new(repo, issues, patches, &notif)? {
                                Some(kind) => Notification::new(&profile, &project, &notif, kind),
                                _ => Ok(None),
                            }
                        }
                        _ => Ok(None),
                    })
                    .filter_map(|notif| notif.ok())
                    .flatten()
                    .collect::<Vec<_>>()
            }
            RepositoryMode::Contextual => {
                let repo = profile.storage.repository(self.context.rid)?;
                let project = repo.project()?;
                let issues = profile.issues(&repo)?;
                let patches = profile.patches(&repo)?;
                let by_repo = notifs.by_repo(&repo.id, "timestamp")?;

                by_repo
                    .filter_map(|notif| notif.ok())
                    .map(
                        |notif| match NotificationKind::new(&repo, &issues, &patches, &notif)? {
                            Some(kind) => Notification::new(&profile, &project, &notif, kind),
                            _ => Ok(None),
                        },
                    )
                    .filter_map(|notif| notif.ok())
                    .flatten()
                    .collect::<Vec<_>>()
            }
            RepositoryMode::ByRepo((rid, _)) => {
                let repo = profile.storage.repository(rid)?;
                let project = repo.project()?;
                let issues = profile.issues(&repo)?;
                let patches = profile.patches(&repo)?;
                let by_repo = notifs.by_repo(&repo.id, "timestamp")?;

                by_repo
                    .filter_map(|notif| notif.ok())
                    .map(
                        |notif| match NotificationKind::new(&repo, &issues, &patches, &notif)? {
                            Some(kind) => Notification::new(&profile, &project, &notif, kind),
                            _ => Ok(None),
                        },
                    )
                    .filter_map(|notif| notif.ok())
                    .flatten()
                    .collect::<Vec<_>>()
            }
        };

        Ok(vec![Message::Loaded(notifications)])
    }
}

impl Process<Message> for Loader {
    async fn process(&mut self, message: Message) -> anyhow::Result<Vec<Message>> {
        match message {
            Message::Initialize | Message::Reload => {
                let loader = NotificationLoader::new(self.context.clone());
                let messages = tokio::spawn(async move { loader.run() }).await.unwrap()?;
                Ok(messages)
            }
            _ => Ok(vec![]),
        }
    }
}
