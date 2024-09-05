use std::str::FromStr;

use anyhow::Result;

use termion::event::Key;

use ratatui::layout::{Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::text::Span;
use ratatui::Frame;

use radicle_tui as tui;

use tui::ui::im::widget::{GroupState, TableState, TextEditState, TextViewState, Window};
use tui::ui::im::{self, Show};
use tui::ui::im::{Borders, BufferedValue};
use tui::ui::Column;
use tui::{store, Exit};

use crate::cob::patch;
use crate::tui_patch::common::{Mode, PatchOperation};
use crate::ui::items::{Filter, PatchItem, PatchItemFilter};

use super::{Context, Selection};

const HELP: &str = r#"# Generic keybindings

`↑,k`:      move cursor one line up
`↓,j:       move cursor one line down
`PageUp`:   move cursor one page up
`PageDown`: move cursor one page down
`Home`:     move cursor to the first line
`End`:      move cursor to the last line
`Esc`:      Quit / cancel

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

#[derive(Clone, Debug)]
pub enum Message {
    Quit,
    Exit {
        operation: Option<PatchOperation>,
    },
    ExitFromMode,
    PatchesChanged {
        state: TableState,
    },
    MainGroupChanged {
        state: GroupState,
    },
    PageChanged {
        page: Page,
    },
    HelpChanged {
        state: TextViewState,
    },
    ShowSearch,
    UpdateSearch {
        search: BufferedValue<TextEditState>,
    },
    HideSearch {
        apply: bool,
    },
}

#[derive(Clone, Debug)]
pub enum Page {
    Main,
    Help,
}

#[derive(Clone, Debug)]
pub struct Storage {
    patches: Vec<PatchItem>,
}

#[derive(Clone, Debug)]
pub struct App {
    storage: Storage,
    mode: Mode,
    page: Page,
    main_group: GroupState,
    patches: TableState,
    search: BufferedValue<TextEditState>,
    show_search: bool,
    help: TextViewState,
    filter: PatchItemFilter,
}

impl App {
    pub fn selected_patch(&self) -> Option<&PatchItem> {
        let patches = self
            .storage
            .patches
            .iter()
            .filter(|patch| self.filter.matches(patch))
            .collect::<Vec<_>>();

        self.patches
            .selected()
            .and_then(|selected| patches.get(selected))
            .copied()
    }
}

impl TryFrom<&Context> for App {
    type Error = anyhow::Error;

    fn try_from(context: &Context) -> Result<Self, Self::Error> {
        let patches = patch::all(&context.profile, &context.repository)?;
        let search = context.filter.to_string();
        let filter = PatchItemFilter::from_str(&context.filter.to_string()).unwrap_or_default();

        let mut items = vec![];
        for patch in patches {
            if let Ok(item) = PatchItem::new(&context.profile, &context.repository, patch.clone()) {
                items.push(item);
            }
        }
        items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(App {
            storage: Storage {
                patches: items.clone(),
            },
            mode: context.mode.clone(),
            page: Page::Main,
            main_group: GroupState::new(3, Some(0)),
            patches: TableState::new(Some(0)),
            search: BufferedValue::new(TextEditState {
                text: search,
                cursor: 0,
            }),
            show_search: false,
            help: TextViewState::new(HELP, (0, 0)),
            filter,
        })
    }
}

impl store::Update<Message> for App {
    type Return = Selection;

    fn update(&mut self, message: Message) -> Option<tui::Exit<Selection>> {
        log::debug!("[State] Received message: {:?}", message);

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
                let operation = match self.mode {
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
            Message::PatchesChanged { state } => {
                self.patches = state;
                None
            }
            Message::MainGroupChanged { state } => {
                self.main_group = state;
                None
            }
            Message::PageChanged { page } => {
                self.page = page;
                None
            }
            Message::ShowSearch => {
                self.main_group = GroupState::new(3, None);
                self.show_search = true;
                None
            }
            Message::HideSearch { apply } => {
                self.main_group = GroupState::new(3, Some(0));
                self.show_search = false;

                if apply {
                    self.search.apply();
                } else {
                    self.search.reset();
                }

                self.filter =
                    PatchItemFilter::from_str(&self.search.read().text).unwrap_or_default();

                None
            }
            Message::UpdateSearch { search } => {
                self.search = search;
                self.filter =
                    PatchItemFilter::from_str(&self.search.read().text).unwrap_or_default();
                self.patches.select_first();
                None
            }
            Message::HelpChanged { state } => {
                self.help = state;
                None
            }
        }
    }
}

impl App {
    pub fn show_patches(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
        let patches = self
            .storage
            .patches
            .iter()
            .filter(|patch| self.filter.matches(patch))
            .cloned()
            .collect::<Vec<_>>();
        let mut selected = self.patches.selected();

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

        let table = ui.headered_table(frame, &mut selected, &patches, header);
        if table.changed {
            ui.send_message(Message::PatchesChanged {
                state: TableState::new(selected),
            });
        }

        // TODO(erikli): Should only work if table has focus
        if ui.input_global(|key| key == Key::Char('/')) {
            ui.send_message(Message::ShowSearch);
        }
    }

    pub fn show_search_text_edit(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
        let (mut search_text, mut search_cursor) = (
            self.search.clone().read().text,
            self.search.clone().read().cursor,
        );
        let mut search = self.search.clone();

        let text_edit = ui.text_edit_labeled_singleline(
            frame,
            &mut search_text,
            &mut search_cursor,
            "Search".to_string(),
            Some(Borders::Spacer { top: 0, left: 0 }),
        );

        if text_edit.changed {
            search.write(TextEditState {
                text: search_text,
                cursor: search_cursor,
            });
            ui.send_message(Message::UpdateSearch { search });
        }

        if ui.input_global(|key| key == Key::Esc) {
            ui.send_message(Message::HideSearch { apply: false });
        }
        if ui.input_global(|key| key == Key::Char('\n')) {
            ui.send_message(Message::HideSearch { apply: true });
        }
    }
}

impl Show<Message> for App {
    fn show(&self, ctx: &im::Context<Message>, frame: &mut Frame) -> Result<()> {
        Window::default().show(ctx, |ui| {
            match self.page {
                Page::Main => {
                    let show_search = self.show_search;
                    let mut page_focus = if show_search { Some(1) } else { Some(0) };
                    let mut group_focus = self.main_group.focus();

                    ui.group(
                        Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]),
                        &mut page_focus,
                        |ui| {
                            let group = ui.group(
                                im::Layout::Expandable3 { left_only: true },
                                &mut group_focus,
                                |ui| {
                                    self.show_patches(frame, ui);

                                    ui.text_view(
                                        frame,
                                        String::new(),
                                        &mut (0, 0),
                                        Some(Borders::All),
                                    );
                                    ui.text_view(
                                        frame,
                                        String::new(),
                                        &mut (0, 0),
                                        Some(Borders::All),
                                    );
                                },
                            );
                            if group.response.changed {
                                ui.send_message(Message::MainGroupChanged {
                                    state: GroupState::new(3, group_focus),
                                });
                            }

                            if show_search {
                                self.show_search_text_edit(frame, ui);
                            } else {
                                ui.layout(Layout::vertical([1, 1]), |ui| {
                                    ui.bar(
                                        frame,
                                        match group_focus {
                                            Some(0) => browser_context(ui, self),
                                            _ => default_context(ui),
                                        },
                                        Some(Borders::None),
                                    );

                                    ui.shortcuts(
                                        frame,
                                        &match self.mode {
                                            Mode::Id => {
                                                [("enter", "select"), ("/", "search")].to_vec()
                                            }
                                            Mode::Operation => [
                                                ("enter", "show"),
                                                ("c", "checkout"),
                                                ("d", "diff"),
                                                ("/", "search"),
                                                ("?", "help"),
                                            ]
                                            .to_vec(),
                                        },
                                        '∙',
                                    );
                                });

                                if ui.input_global(|key| key == Key::Esc) {
                                    ui.send_message(Message::Quit);
                                }
                                if ui.input_global(|key| key == Key::Char('?')) {
                                    ui.send_message(Message::PageChanged { page: Page::Help });
                                }
                                if ui.input_global(|key| key == Key::Char('\n')) {
                                    ui.send_message(Message::ExitFromMode);
                                }
                                if ui.input_global(|key| key == Key::Char('d')) {
                                    ui.send_message(Message::Exit {
                                        operation: Some(PatchOperation::Diff),
                                    });
                                }
                                if ui.input_global(|key| key == Key::Char('c')) {
                                    ui.send_message(Message::Exit {
                                        operation: Some(PatchOperation::Checkout),
                                    });
                                }
                            }
                        },
                    );
                }

                Page::Help => {
                    let mut cursor = self.help.cursor();

                    let layout = Layout::vertical([
                        Constraint::Length(3),
                        Constraint::Fill(1),
                        Constraint::Length(1),
                        Constraint::Length(1),
                    ]);

                    ui.layout(layout, |ui| {
                        ui.set_focus(Some(0));
                        ui.columns(
                            frame,
                            [Column::new(Span::raw(" Help ").bold(), Constraint::Fill(1))].to_vec(),
                            Some(Borders::Top),
                        );

                        ui.set_focus(Some(1));
                        let text_view = ui.text_view(
                            frame,
                            self.help.text().to_string(),
                            &mut cursor,
                            Some(Borders::BottomSides),
                        );
                        if text_view.changed {
                            ui.send_message(Message::HelpChanged {
                                state: TextViewState::new(self.help.text().to_string(), cursor),
                            })
                        }

                        ui.bar(
                            frame,
                            [
                                Column::new(
                                    Span::raw(" ".to_string())
                                        .into_left_aligned_line()
                                        .style(ui.theme.bar_on_black_style),
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
                            Some(Borders::None),
                        );

                        ui.shortcuts(frame, &[("?", "close")], '∙');
                    });

                    if ui.input_global(|key| key == Key::Char('?')) {
                        ui.send_message(Message::PageChanged { page: Page::Main });
                    }
                    if ui.input_global(|key| key == Key::Esc) {
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

fn browser_context<'a>(ui: &im::Ui<Message>, app: &'a App) -> Vec<Column<'a>> {
    let search = app.search.read().text;
    let total_count = app.storage.patches.len();
    let filtered_count = app
        .storage
        .patches
        .iter()
        .filter(|patch| app.filter.matches(patch))
        .collect::<Vec<_>>()
        .len();
    let experimental = false;

    if experimental {
        [
            Column::new(
                Span::raw(" Search ".to_string()).cyan().dim().reversed(),
                Constraint::Length(8),
            ),
            Column::new(Span::raw("".to_string()), Constraint::Length(1)),
            Column::new(
                Span::raw(format!(" {} ", search))
                    .into_left_aligned_line()
                    .cyan()
                    .dim()
                    .reversed(),
                Constraint::Length((search.chars().count() + 2) as u16),
            ),
            Column::new(Span::raw("".to_string()), Constraint::Fill(1)),
            Column::new(
                Span::raw(" 0% ")
                    .into_right_aligned_line()
                    .red()
                    .dim()
                    .reversed(),
                Constraint::Length(6),
            ),
        ]
        .to_vec()
    } else {
        let filtered_counts = format!(" {filtered_count}/{total_count} ");
        let state_counts =
            app.storage
                .patches
                .iter()
                .fold((0, 0, 0, 0), |counts, patch| match patch.state {
                    radicle::patch::State::Draft => (counts.0 + 1, counts.1, counts.2, counts.3),
                    radicle::patch::State::Open { conflicts: _ } => {
                        (counts.0, counts.1 + 1, counts.2, counts.3)
                    }
                    radicle::patch::State::Archived => (counts.0, counts.1, counts.2 + 1, counts.3),
                    radicle::patch::State::Merged {
                        revision: _,
                        commit: _,
                    } => (counts.0, counts.1, counts.2, counts.3 + 1),
                });

        if app.filter.is_default() {
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
                        .style(ui.theme.bar_on_black_style),
                    Constraint::Fill(1),
                ),
                Column::new(
                    Span::raw("●")
                        .style(ui.theme.bar_on_black_style)
                        .dim()
                        .bold(),
                    Constraint::Length(1),
                ),
                Column::new(
                    Span::raw(draft.clone())
                        .style(ui.theme.bar_on_black_style)
                        .dim(),
                    Constraint::Length(draft.chars().count() as u16),
                ),
                Column::new(
                    Span::raw("●")
                        .style(ui.theme.bar_on_black_style)
                        .green()
                        .dim()
                        .bold(),
                    Constraint::Length(1),
                ),
                Column::new(
                    Span::raw(open.clone())
                        .style(ui.theme.bar_on_black_style)
                        .dim(),
                    Constraint::Length(open.chars().count() as u16),
                ),
                Column::new(
                    Span::raw("●")
                        .style(ui.theme.bar_on_black_style)
                        .yellow()
                        .dim()
                        .bold(),
                    Constraint::Length(1),
                ),
                Column::new(
                    Span::raw(archived.clone())
                        .style(ui.theme.bar_on_black_style)
                        .dim(),
                    Constraint::Length(archived.chars().count() as u16),
                ),
                Column::new(
                    Span::raw("✔")
                        .style(ui.theme.bar_on_black_style)
                        .magenta()
                        .dim()
                        .bold(),
                    Constraint::Length(1),
                ),
                Column::new(
                    Span::raw(merged.clone())
                        .style(ui.theme.bar_on_black_style)
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
                        .style(ui.theme.bar_on_black_style),
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
    }
}

fn default_context<'a>(ui: &im::Ui<Message>) -> Vec<Column<'a>> {
    [
        Column::new(
            Span::raw(" ".to_string())
                .into_left_aligned_line()
                .style(ui.theme.bar_on_black_style),
            Constraint::Fill(1),
        ),
        Column::new(
            Span::raw(" 0% ")
                .into_right_aligned_line()
                .cyan()
                .dim()
                .reversed(),
            Constraint::Length(6),
        ),
    ]
    .to_vec()
}
