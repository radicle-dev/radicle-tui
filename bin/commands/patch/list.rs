use std::str::FromStr;
use std::sync::{Arc, Mutex};

use anyhow::Result;

use termion::event::Key;

use radicle::patch::PatchId;
use radicle::storage::git::Repository;
use radicle::Profile;

use ratatui::layout::{Constraint, Layout, Position};
use ratatui::style::Stylize;
use ratatui::text::Span;
use ratatui::{Frame, Viewport};

use radicle_tui as tui;

use tui::store;
use tui::task::EmptyProcessors;
use tui::ui::im;
use tui::ui::im::widget::{PanesState, TableState, TextEditState, TextViewState, Window};
use tui::ui::im::Borders;
use tui::ui::im::Show;
use tui::ui::{BufferedValue, Column, Spacing};
use tui::{Channel, Exit};

type Selection = tui::Selection<PatchId>;

use super::common::{Mode, PatchOperation};

use crate::cob::patch;
use crate::ui::items::filter::Filter;
use crate::ui::items::{PatchItem, PatchItemFilter};

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

`enter`:    Select patch (if --mode id)
`enter`:    Show patch
`c`:        Checkout patch
`d`:        Show patch diff
`/`:        Search
`?`:        Show help

# Searching

Pattern:    is:<state> | is:authored | authors:[<did>, <did>] | <search>
Example:    is:open is:authored improve"#;

pub struct Context {
    pub profile: Profile,
    pub repository: Repository,
    pub mode: Mode,
    pub filter: patch::Filter,
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
        state: PanesState,
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
    ExitFromMode,
    Quit,
}

#[derive(Clone, Debug)]
pub enum Page {
    Main,
    Help,
}

#[derive(Clone, Debug)]
pub struct AppState {
    mode: Mode,
    page: Page,
    main_group: PanesState,
    patches: TableState,
    search: BufferedValue<TextEditState>,
    show_search: bool,
    help: TextViewState,
    filter: PatchItemFilter,
}

#[derive(Clone, Debug)]
pub struct App {
    patches: Arc<Mutex<Vec<PatchItem>>>,
    state: AppState,
}

impl TryFrom<&Context> for App {
    type Error = anyhow::Error;

    fn try_from(context: &Context) -> Result<Self, Self::Error> {
        let patches = patch::all(&context.profile, &context.repository)?;
        let search = {
            let raw = context.filter.to_string();
            raw.trim().to_string()
        };
        let filter = PatchItemFilter::from_str(&context.filter.to_string()).unwrap_or_default();

        let mut items = patches
            .into_iter()
            .flat_map(|patch| {
                PatchItem::new(&context.profile, &context.repository, patch.clone()).ok()
            })
            .collect::<Vec<_>>();

        items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(App {
            patches: Arc::new(Mutex::new(items.clone())),
            state: AppState {
                mode: context.mode.clone(),
                page: Page::Main,
                main_group: PanesState::new(3, Some(0)),
                patches: TableState::new(Some(0)),
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
            Message::Exit { operation } => self.selected_patch().map(|issue| Exit {
                value: Some(Selection {
                    operation: operation.map(|op| op.to_string()),
                    ids: vec![issue.id],
                    args: vec![],
                }),
            }),
            Message::ExitFromMode => {
                let operation = match self.state.mode {
                    Mode::Operation => Some(PatchOperation::Show.to_string()),
                    Mode::Id => None,
                };

                self.selected_patch().map(|issue| Exit {
                    value: Some(Selection {
                        operation,
                        ids: vec![issue.id],
                        args: vec![],
                    }),
                })
            }
            Message::ShowSearch => {
                self.state.main_group = PanesState::new(3, None);
                self.state.show_search = true;
                None
            }
            Message::HideSearch { apply } => {
                self.state.main_group = PanesState::new(3, Some(0));
                self.state.show_search = false;

                if apply {
                    self.state.search.apply();
                } else {
                    self.state.search.reset();
                }

                self.state.filter =
                    PatchItemFilter::from_str(&self.state.search.read().text).unwrap_or_default();

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
                    self.state.filter = PatchItemFilter::from_str(&self.state.search.read().text)
                        .unwrap_or_default();
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
    fn show(&self, ctx: &im::Context<Message>, frame: &mut Frame) -> Result<()> {
        Window::default().show(ctx, |ui| {
            match self.state.page {
                Page::Main => {
                    let show_search = self.state.show_search;
                    let mut page_focus = if show_search { Some(1) } else { Some(0) };

                    ui.panes(
                        Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]),
                        &mut page_focus,
                        |ui| {
                            let mut group_focus = self.state.main_group.focus();

                            let group = ui.panes(
                                im::Layout::Expandable3 { left_only: true },
                                &mut group_focus,
                                |ui| {
                                    self.show_browser(frame, ui);
                                },
                            );
                            if group.response.changed {
                                ui.send_message(Message::Changed(Change::MainGroup {
                                    state: PanesState::new(3, group_focus),
                                }));
                            }

                            if show_search {
                                self.show_browser_search(frame, ui);
                            } else if let Some(0) = group_focus {
                                self.show_browser_footer(frame, ui);
                            }
                        },
                    );
                }

                Page::Help => {
                    let layout = Layout::vertical([
                        Constraint::Length(3),
                        Constraint::Fill(1),
                        Constraint::Length(1),
                        Constraint::Length(1),
                    ]);

                    ui.container(layout, 1, |ui| {
                        self.show_help_text(frame, ui);
                        self.show_help_context(frame, ui);

                        ui.shortcuts(frame, &[("?", "close")], '∙');
                    });

                    if ui.input_global(|key| key == Key::Char('?')) {
                        ui.send_message(Message::Changed(Change::Page { page: Page::Main }));
                    }
                    if ui.input_global(|key| key == Key::Char('q')) {
                        ui.send_message(Message::Quit);
                    }
                }
            }
            if ui.input_global(|key| key == Key::Ctrl('c')) {
                ui.send_message(Message::Quit);
            }
        });

        Ok(())
    }
}

impl App {
    pub fn show_browser(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
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
                ui.column_bar(
                    frame,
                    header.to_vec(),
                    Spacing::default(),
                    Some(Borders::Top),
                );

                let table = ui.table(
                    frame,
                    &mut selected,
                    &patches,
                    header.to_vec(),
                    Some("No patches found".into()),
                    Some(Borders::BottomSides),
                );
                if table.changed {
                    ui.send_message(Message::Changed(Change::Patches {
                        state: TableState::new(selected),
                    }));
                }

                // TODO(erikli): Should only work if table has focus
                if ui.input_global(|key| key == Key::Char('/')) {
                    ui.send_message(Message::ShowSearch);
                }
            },
        );
    }

    fn show_browser_footer(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
        ui.layout(Layout::vertical([1, 1]), None, |ui| {
            self.show_browser_context(frame, ui);
            self.show_browser_shortcuts(frame, ui);
        });
        if ui.input_global(|key| key == Key::Char('q')) {
            ui.send_message(Message::Quit);
        }
        if ui.input_global(|key| key == Key::Char('?')) {
            ui.send_message(Message::Changed(Change::Page { page: Page::Help }));
        }
        if ui.input_global(|key| key == Key::Char('\n')) {
            ui.send_message(Message::ExitFromMode);
        }
        if ui.input_global(|key| key == Key::Char('d')) {
            ui.send_message(Message::Exit {
                operation: Some(PatchOperation::Diff),
            });
        }
        if ui.input_global(|key| key == Key::Char('r')) {
            ui.send_message(Message::Exit {
                operation: Some(PatchOperation::Review),
            });
        }
        if ui.input_global(|key| key == Key::Char('c')) {
            ui.send_message(Message::Exit {
                operation: Some(PatchOperation::Checkout),
            });
        }
    }

    pub fn show_browser_search(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
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

        if ui.input_global(|key| key == Key::Esc) {
            ui.send_message(Message::HideSearch { apply: false });
        }
        if ui.input_global(|key| key == Key::Char('\n')) {
            ui.send_message(Message::HideSearch { apply: true });
        }
    }

    fn show_browser_context(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
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

            if self.state.filter.is_default() {
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
                            .style(ui.theme().bar_on_black_style),
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
                            .style(ui.theme().bar_on_black_style),
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

    pub fn show_browser_shortcuts(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
        ui.shortcuts(
            frame,
            &match self.state.mode {
                Mode::Id => [("enter", "select"), ("/", "search")].to_vec(),
                Mode::Operation => [
                    ("enter", "show"),
                    ("c", "checkout"),
                    ("d", "diff"),
                    ("r", "review"),
                    ("/", "search"),
                    ("?", "help"),
                ]
                .to_vec(),
            },
            '∙',
        );
    }

    fn show_help_text(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
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

    fn show_help_context(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
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

    pub fn selected_patch(&self) -> Option<PatchItem> {
        let patches = self.patches.lock().unwrap();
        match self.state.patches.selected() {
            Some(selected) => patches
                .iter()
                .filter(|patch| self.state.filter.matches(patch))
                .collect::<Vec<_>>()
                .get(selected)
                .cloned()
                .cloned(),
            _ => None,
        }
    }
}
