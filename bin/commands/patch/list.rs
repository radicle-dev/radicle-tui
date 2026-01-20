use std::str::FromStr;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};

use serde::Serialize;

use radicle::patch::cache::Patches;
use radicle::patch::PatchId;
use radicle::storage::git::Repository;
use radicle::Profile;

use ratatui::layout::{Alignment, Constraint, Layout, Position};
use ratatui::style::Stylize;
use ratatui::text::Span;
use ratatui::{Frame, Viewport};

use radicle_tui as tui;

use tui::event::Key;
use tui::store;
use tui::task::EmptyProcessors;
use tui::ui;
use tui::ui::layout::Spacing;
use tui::ui::widget::{
    Borders, Column, ContainerState, TableState, TextEditState, TextViewState, Window,
};
use tui::ui::{BufferedValue, Show, Ui};
use tui::{Channel, Exit};

use crate::ui::items::filter::Filter;
use crate::ui::items::patch::filter::PatchFilter;
use crate::ui::items::patch::Patch;

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

`enter`:    Show patch
`c`:        Checkout patch
`d`:        Show patch diff
`/`:        Search
`?`:        Show help

# Searching

Examples:   state=open bugfix
            state=merged author=(did:key:... or did:key:...)"#;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OperationArguments {
    id: PatchId,
    search: String,
}

impl OperationArguments {
    pub fn id(&self) -> PatchId {
        self.id
    }

    pub fn search(&self) -> String {
        self.search.clone()
    }
}

impl TryFrom<(&Vec<Patch>, &AppState)> for OperationArguments {
    type Error = anyhow::Error;

    fn try_from(value: (&Vec<Patch>, &AppState)) -> Result<Self> {
        let (patches, state) = value;
        let selected = state.patches.selected();
        let id = selected
            .and_then(|s| patches.get(s))
            .ok_or(anyhow!("No patch selected"))?
            .id;
        let search = state.search.read().text;

        Ok(Self { id, search })
    }
}

/// The selected patch operation returned by the operation
/// selection widget.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum PatchOperation {
    Checkout { args: OperationArguments },
    Diff { args: OperationArguments },
    Show { args: OperationArguments },
    _Review { args: OperationArguments },
}

type Selection = tui::Selection<PatchOperation>;

pub struct Context {
    pub profile: Profile,
    pub repository: Repository,
    pub filter: PatchFilter,
    pub patch_id: Option<PatchId>,
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

        tui::im(state, viewport, channel, EmptyProcessors::new()).await
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
    Changed(Change),
    ShowSearch,
    HideSearch { apply: bool },
    Exit { operation: Option<PatchOperation> },
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
    filter: PatchFilter,
}

#[derive(Clone, Debug)]
pub struct App {
    patches: Arc<Mutex<Vec<Patch>>>,
    state: AppState,
}

impl TryFrom<&Context> for App {
    type Error = anyhow::Error;

    fn try_from(context: &Context) -> Result<Self, Self::Error> {
        let cache = &context.profile.patches(&context.repository)?;
        let mut patches = cache
            .list()?
            .filter_map(|patch| patch.ok())
            .flat_map(|patch| Patch::without_stats(&context.profile, patch.clone()).ok())
            .collect::<Vec<_>>();
        patches.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        let search = context.search.as_ref().map(|s| s.trim().to_string());
        let (search, filter) = match search {
            Some(search) => (
                search.clone(),
                PatchFilter::from_str(search.trim()).unwrap_or(PatchFilter::Invalid),
            ),
            None => {
                let filter = context.filter.clone();
                (filter.to_string().trim().to_string(), filter)
            }
        };

        Ok(App {
            patches: Arc::new(Mutex::new(patches.clone())),
            state: AppState {
                page: Page::Main,
                main_group: ContainerState::new(3, Some(0)),
                patches: TableState::new(Some(
                    context
                        .patch_id
                        .and_then(|id| {
                            patches
                                .iter()
                                .filter(|item| filter.matches(item))
                                .position(|item| item.id == id)
                        })
                        .unwrap_or(0),
                )),
                search: BufferedValue::new(TextEditState {
                    text: search.clone(),
                    cursor: search.len(),
                }),
                show_search: false,
                help: TextViewState::new(Position::default()),
                filter,
            },
        })
    }
}

impl store::Update<Message> for App {
    type Return = Selection;

    fn update(&mut self, message: Message) -> Option<tui::Exit<Selection>> {
        match message {
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

                self.state.filter = PatchFilter::from_str(&self.state.search.read().text)
                    .unwrap_or(PatchFilter::Invalid);

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
                    self.state.filter = PatchFilter::from_str(&self.state.search.read().text)
                        .unwrap_or(PatchFilter::Invalid);
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
        let patches = self.patches.lock().unwrap();
        let patches = patches
            .iter()
            .filter(|patch| self.state.filter.matches(patch))
            .cloned()
            .collect::<Vec<_>>();
        let mut selected = self.state.patches.selected();

        let header = [
            Column::new(Span::raw(" ● ").bold(), Constraint::Length(3)),
            Column::new(Span::raw("ID").bold(), Constraint::Length(8)),
            Column::new(Span::raw("Title").bold(), Constraint::Fill(1)),
            Column::new(Span::raw("Author").bold(), Constraint::Length(16)).hide_small(),
            Column::new("", Constraint::Length(16)).hide_medium(),
            Column::new(Span::raw("Head").bold(), Constraint::Length(8)).hide_small(),
            Column::new(Span::raw("+").bold(), Constraint::Length(6)).hide_small(),
            Column::new(Span::raw("-").bold(), Constraint::Length(6)).hide_small(),
            Column::new(Span::raw("Updated").bold(), Constraint::Length(16)).hide_small(),
        ];

        ui.layout(
            Layout::vertical([Constraint::Length(3), Constraint::Min(1)]),
            Some(1),
            |ui| {
                ui.column_bar(frame, header.to_vec(), Spacing::from(1), Some(Borders::Top));

                let table = ui.table(
                    frame,
                    &mut selected,
                    &patches,
                    header.to_vec(),
                    Some("No patches found".into()),
                    Spacing::from(1),
                    Some(Borders::BottomSides),
                );
                if table.changed {
                    ui.send_message(Message::Changed(Change::Patches {
                        state: TableState::new(selected),
                    }));
                }
            },
        );

        // TODO(erikli): Should only work if table has focus
        if ui.has_input(|key| key == Key::Char('/')) {
            ui.send_message(Message::ShowSearch);
        }

        if let Ok(args) = OperationArguments::try_from((&patches, &self.state)) {
            if ui.has_input(|key| key == Key::Enter) {
                ui.send_message(Message::Exit {
                    operation: Some(PatchOperation::Show { args: args.clone() }),
                });
            }
            if ui.has_input(|key| key == Key::Char('d')) {
                ui.send_message(Message::Exit {
                    operation: Some(PatchOperation::Diff { args: args.clone() }),
                });
            }
            if ui.has_input(|key| key == Key::Char('c')) {
                ui.send_message(Message::Exit {
                    operation: Some(PatchOperation::Checkout { args }),
                });
            }
        }
    }

    fn show_browser_footer(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        ui.layout(Layout::vertical([1, 1]), None, |ui| {
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
            let patches = self.patches.lock().unwrap();
            let search = self.state.search.read().text;
            let total_count = patches.len();
            let filtered_count = patches
                .iter()
                .filter(|patch| self.state.filter.matches(patch))
                .collect::<Vec<_>>()
                .len();

            let filtered_counts = format!(" {filtered_count}/{total_count} ");
            let state_counts =
                patches
                    .iter()
                    .fold((0, 0, 0, 0), |counts, patch| match patch.state {
                        radicle::patch::State::Draft => {
                            (counts.0 + 1, counts.1, counts.2, counts.3)
                        }
                        radicle::patch::State::Open { conflicts: _ } => {
                            (counts.0, counts.1 + 1, counts.2, counts.3)
                        }
                        radicle::patch::State::Archived => {
                            (counts.0, counts.1, counts.2 + 1, counts.3)
                        }
                        radicle::patch::State::Merged {
                            revision: _,
                            commit: _,
                        } => (counts.0, counts.1, counts.2, counts.3 + 1),
                    });

            if !self.state.filter.has_state() {
                let draft = format!(" {} ", state_counts.0);
                let open = format!(" {} ", state_counts.1);
                let archived = format!(" {} ", state_counts.2);
                let merged = format!(" {} ", state_counts.3);
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
                            .dim()
                            .bold(),
                        Constraint::Length(1),
                    ),
                    Column::new(
                        Span::raw(draft.clone())
                            .style(ui.theme().bar_on_black_style)
                            .dim(),
                        Constraint::Length(draft.chars().count() as u16),
                    ),
                    Column::new(
                        Span::raw("●")
                            .style(ui.theme().bar_on_black_style)
                            .green()
                            .dim()
                            .bold(),
                        Constraint::Length(1),
                    ),
                    Column::new(
                        Span::raw(open.clone())
                            .style(ui.theme().bar_on_black_style)
                            .dim(),
                        Constraint::Length(open.chars().count() as u16),
                    ),
                    Column::new(
                        Span::raw("●")
                            .style(ui.theme().bar_on_black_style)
                            .yellow()
                            .dim()
                            .bold(),
                        Constraint::Length(1),
                    ),
                    Column::new(
                        Span::raw(archived.clone())
                            .style(ui.theme().bar_on_black_style)
                            .dim(),
                        Constraint::Length(archived.chars().count() as u16),
                    ),
                    Column::new(
                        Span::raw("✔")
                            .style(ui.theme().bar_on_black_style)
                            .magenta()
                            .dim()
                            .bold(),
                        Constraint::Length(1),
                    ),
                    Column::new(
                        Span::raw(merged.clone())
                            .style(ui.theme().bar_on_black_style)
                            .dim(),
                        Constraint::Length(merged.chars().count() as u16),
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
                            .cyan()
                            .dim(),
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
                ("c", "checkout"),
                ("d", "diff"),
                ("/", "search"),
                ("?", "help"),
            ],
            '∙',
            Alignment::Left,
        );
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
