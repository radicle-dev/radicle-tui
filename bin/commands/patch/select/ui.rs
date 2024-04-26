use std::any::Any;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::vec;

use ratatui::widgets::TableState;
use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};

use radicle::patch::{self, Status};

use radicle_tui as tui;

use tui::ui::items::{ItemView, PatchItem, PatchItemFilter};
use tui::ui::span;
use tui::ui::widget::container::{
    Container, ContainerProps, Footer, FooterProps, Header, HeaderProps,
};
use tui::ui::widget::input::{TextField, TextFieldProps, TextFieldState};
use tui::ui::widget::text::{Paragraph, ParagraphProps, ParagraphState};
use tui::ui::widget::{self, BaseView};
use tui::ui::widget::{Column, Properties, Shortcuts, ShortcutsProps, Table, TableProps, Widget};
use tui::ui::widget::{TableUtils, WidgetState};
use tui::Selection;

use crate::tui_patch::common::Mode;
use crate::tui_patch::common::PatchOperation;

use super::{Action, State};

type BoxedWidget = widget::BoxedWidget<State, Action>;

#[derive(Clone)]
pub struct BrowsePageProps<'a> {
    mode: Mode,
    patches: Arc<ItemView<PatchItem, PatchItemFilter>>,
    selected: Option<usize>,
    search: String,
    stats: HashMap<String, usize>,
    columns: Vec<Column<'a>>,
    cutoff: usize,
    cutoff_after: usize,
    focus: bool,
    page_size: usize,
    show_search: bool,
    shortcuts: Vec<(&'a str, &'a str)>,
}

impl<'a> From<&State> for BrowsePageProps<'a> {
    fn from(state: &State) -> Self {
        let mut draft = 0;
        let mut open = 0;
        let mut archived = 0;
        let mut merged = 0;

        let patches = state.browser.patches.list().collect::<Vec<_>>();

        for patch in patches {
            match patch.state {
                patch::State::Draft => draft += 1,
                patch::State::Open { conflicts: _ } => open += 1,
                patch::State::Archived => archived += 1,
                patch::State::Merged {
                    commit: _,
                    revision: _,
                } => merged += 1,
            }
        }

        let stats = HashMap::from([
            ("Draft".to_string(), draft),
            ("Open".to_string(), open),
            ("Archived".to_string(), archived),
            ("Merged".to_string(), merged),
        ]);

        Self {
            mode: state.mode.clone(),
            patches: state.browser.patches.clone(),
            search: state.browser.search.read(),
            columns: [
                Column::new(" ● ", Constraint::Length(3)),
                Column::new("ID", Constraint::Length(8)),
                Column::new("Title", Constraint::Fill(1)),
                Column::new("Author", Constraint::Length(16)),
                Column::new("", Constraint::Length(16)),
                Column::new("Head", Constraint::Length(8)),
                Column::new("+", Constraint::Length(6)),
                Column::new("-", Constraint::Length(6)),
                Column::new("Updated", Constraint::Length(16)),
            ]
            .to_vec(),
            cutoff: 150,
            cutoff_after: 5,
            focus: false,
            stats,
            page_size: state.browser.page_size,
            show_search: state.browser.show_search,
            selected: state.browser.selected,
            shortcuts: match state.mode {
                Mode::Id => vec![("enter", "select"), ("/", "search")],
                Mode::Operation => vec![
                    ("enter", "show"),
                    ("c", "checkout"),
                    ("d", "diff"),
                    ("/", "search"),
                    ("?", "help"),
                ],
            },
        }
    }
}

impl<'a: 'static> Properties for BrowsePageProps<'a> {}

pub struct BrowsePage<'a> {
    /// Internal base
    base: BaseView<State, Action>,
    /// Internal props
    props: BrowsePageProps<'a>,
    /// Notifications widget
    patches: BoxedWidget,
    /// Search widget
    search: BoxedWidget,
    /// Shortcut widget
    shortcuts: BoxedWidget,
}

impl<'a: 'static> Widget for BrowsePage<'a> {
    type Action = Action;
    type State = State;

    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self {
        let props = BrowsePageProps::from(state);

        Self {
            base: BaseView {
                action_tx: action_tx.clone(),
                on_update: None,
                on_event: None,
            },
            props: props.clone(),
            patches: Container::new(state, action_tx.clone())
                .header(
                    Header::new(state, action_tx.clone())
                        .columns(props.columns.clone())
                        .cutoff(props.cutoff, props.cutoff_after)
                        .focus(props.focus)
                        .to_boxed(),
                )
                .content(
                    Box::<Table<State, Action, PatchItem, PatchItemFilter, 9>>::new(
                        Table::new(state, action_tx.clone())
                            .on_event(|table, action_tx| {
                                TableState::from_boxed_any(table).and_then(|table| {
                                    action_tx
                                        .send(Action::Select {
                                            selected: table.selected(),
                                        })
                                        .ok()
                                });
                            })
                            .on_update(|state| {
                                let props = BrowsePageProps::from(state);

                                TableProps::default()
                                    .columns(props.columns)
                                    .items(state.browser.patches.clone())
                                    .footer(!state.browser.show_search)
                                    .page_size(state.browser.page_size)
                                    .cutoff(props.cutoff, props.cutoff_after)
                                    .to_boxed()
                            }),
                    ),
                )
                .footer(
                    Footer::new(state, action_tx.clone())
                        .on_update(|state| {
                            let props = BrowsePageProps::from(state);

                            FooterProps::default()
                                .columns(browse_footer(&props, props.selected))
                                .to_boxed()
                        })
                        .to_boxed(),
                )
                .to_boxed(),
            search: Search::new(state, action_tx.clone()).to_boxed(),
            shortcuts: Shortcuts::new(state, action_tx.clone()).to_boxed(),
        }
    }

    fn handle_event(&mut self, key: Key) {
        if self.props.show_search {
            self.search.handle_event(key);
        } else {
            match key {
                Key::Esc | Key::Ctrl('c') => {
                    let _ = self.base.action_tx.send(Action::Exit { selection: None });
                }
                Key::Char('?') => {
                    let _ = self.base.action_tx.send(Action::OpenHelp);
                }
                Key::Char('/') => {
                    let _ = self.base.action_tx.send(Action::OpenSearch);
                }
                Key::Char('\n') => {
                    let operation = match self.props.mode {
                        Mode::Operation => Some(PatchOperation::Show.to_string()),
                        Mode::Id => None,
                    };
                    let patches = self.props.patches.list().collect::<Vec<_>>();
                    self.props
                        .selected
                        .and_then(|selected| patches.get(selected))
                        .and_then(|patch| {
                            self.base
                                .action_tx
                                .send(Action::Exit {
                                    selection: Some(Selection {
                                        operation,
                                        ids: vec![patch.id],
                                        args: vec![],
                                    }),
                                })
                                .ok()
                        });
                }
                Key::Char('c') => {
                    let patches = self.props.patches.list().collect::<Vec<_>>();
                    self.props
                        .selected
                        .and_then(|selected| patches.get(selected))
                        .and_then(|patch| {
                            self.base
                                .action_tx
                                .send(Action::Exit {
                                    selection: Some(Selection {
                                        operation: Some(PatchOperation::Checkout.to_string()),
                                        ids: vec![patch.id],
                                        args: vec![],
                                    }),
                                })
                                .ok()
                        });
                }
                Key::Char('d') => {
                    let patches = self.props.patches.list().collect::<Vec<_>>();
                    self.props
                        .selected
                        .and_then(|selected| patches.get(selected))
                        .and_then(|patch| {
                            self.base
                                .action_tx
                                .send(Action::Exit {
                                    selection: Some(Selection {
                                        operation: Some(PatchOperation::Diff.to_string()),
                                        ids: vec![patch.id],
                                        args: vec![],
                                    }),
                                })
                                .ok()
                        });
                }
                _ => {
                    self.patches.handle_event(key);
                }
            }
        }
    }

    fn update(&mut self, state: &State) {
        self.props = BrowsePageProps::from_callback(self.base.on_update, state)
            .unwrap_or(BrowsePageProps::from(state));

        self.patches.update(state);
        self.search.update(state);
        self.shortcuts.update(state);
    }

    fn render(&self, frame: &mut ratatui::Frame, area: Rect, props: Option<&dyn Any>) {
        let props = props
            .and_then(|props| props.downcast_ref::<BrowsePageProps>())
            .unwrap_or(&self.props);

        let page_size = area.height.saturating_sub(6) as usize;

        let [content_area, shortcuts_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);

        if props.show_search {
            let [table_area, search_area] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(content_area);

            self.patches.render(
                frame,
                table_area,
                Some(&ContainerProps::default().hide_footer(props.show_search)),
            );
            self.search.render(frame, search_area, None);
        } else {
            self.patches.render(
                frame,
                content_area,
                Some(&ContainerProps::default().hide_footer(props.show_search)),
            );
        }

        self.shortcuts.render(
            frame,
            shortcuts_area,
            Some(&ShortcutsProps::default().shortcuts(&props.shortcuts)),
        );

        if page_size != props.page_size {
            let _ = self.base.action_tx.send(Action::BrowserPageSize(page_size));
        }
    }

    fn base_mut(&mut self) -> &mut BaseView<State, Action> {
        &mut self.base
    }
}

pub struct SearchProps {}

impl Properties for SearchProps {}

pub struct Search {
    /// Internal base
    base: BaseView<State, Action>,
    /// Internal props
    _props: SearchProps,
    /// Search input field
    input: BoxedWidget,
}

impl Widget for Search {
    type Action = Action;
    type State = State;

    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let input = TextField::new(state, action_tx.clone())
            .on_event(|field, action_tx| {
                TextFieldState::from_boxed_any(field).and_then(|field| {
                    action_tx
                        .send(Action::UpdateSearch {
                            value: field.text.clone().unwrap_or_default(),
                        })
                        .ok()
                });
            })
            .on_update(|state| {
                TextFieldProps::default()
                    .text(&state.browser.search.read().to_string())
                    .title("Search")
                    .inline(true)
                    .to_boxed()
            })
            .to_boxed();
        Self {
            base: BaseView {
                action_tx: action_tx.clone(),
                on_update: None,
                on_event: None,
            },
            _props: SearchProps {},
            input,
        }
    }

    fn handle_event(&mut self, key: termion::event::Key) {
        match key {
            Key::Esc => {
                let _ = self.base.action_tx.send(Action::CloseSearch);
            }
            Key::Char('\n') => {
                let _ = self.base.action_tx.send(Action::ApplySearch);
            }
            _ => {
                self.input.handle_event(key);
            }
        }
    }

    fn update(&mut self, state: &State) {
        self.input.update(state);
    }

    fn render(&self, frame: &mut ratatui::Frame, area: Rect, _props: Option<&dyn Any>) {
        let layout = Layout::horizontal(Constraint::from_mins([0]))
            .horizontal_margin(1)
            .split(area);

        self.input.render(frame, layout[0], None);
    }

    fn base_mut(&mut self) -> &mut BaseView<State, Action> {
        &mut self.base
    }
}

#[derive(Clone)]
pub struct HelpPageProps<'a> {
    focus: bool,
    page_size: usize,
    help_progress: usize,
    shortcuts: Vec<(&'a str, &'a str)>,
}

impl<'a> From<&State> for HelpPageProps<'a> {
    fn from(state: &State) -> Self {
        Self {
            focus: false,
            page_size: state.help.page_size,
            help_progress: state.help.progress,
            shortcuts: vec![("?", "close")],
        }
    }
}

impl<'a> Properties for HelpPageProps<'a> {}

pub struct HelpPage<'a> {
    /// Internal base
    base: BaseView<State, Action>,
    /// Internal props
    props: HelpPageProps<'a>,
    /// Content widget
    content: BoxedWidget,
    /// Shortcut widget
    shortcuts: BoxedWidget,
}

impl<'a: 'static> Widget for HelpPage<'a> {
    type Action = Action;
    type State = State;

    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            base: BaseView {
                action_tx: action_tx.clone(),
                on_update: None,
                on_event: None,
            },
            props: HelpPageProps::from(state),
            content: Container::new(state, action_tx.clone())
                .header(
                    Header::new(state, action_tx.clone())
                        .on_update(|state| {
                            let props = HelpPageProps::from(state);

                            HeaderProps::default()
                                .columns([Column::new(" Help ", Constraint::Fill(1))].to_vec())
                                .focus(props.focus)
                                .to_boxed()
                        })
                        .to_boxed(),
                )
                .content(
                    Paragraph::new(state, action_tx.clone())
                        .on_update(|state| {
                            let props = HelpPageProps::from(state);

                            ParagraphProps::default()
                                .text(&help_text())
                                .page_size(props.page_size)
                                .focus(props.focus)
                                .to_boxed()
                        })
                        .on_event(|paragraph, action_tx| {
                            ParagraphState::from_boxed_any(paragraph).and_then(|paragraph| {
                                action_tx
                                    .send(Action::ScrollHelp {
                                        progress: paragraph.progress,
                                    })
                                    .ok()
                            });
                        })
                        .to_boxed(),
                )
                .footer(
                    Footer::new(state, action_tx.clone())
                        .on_update(|state| {
                            let props = HelpPageProps::from(state);

                            FooterProps::default()
                                .columns(
                                    [
                                        Column::new(Text::raw(""), Constraint::Fill(1)),
                                        Column::new(
                                            span::default(&format!("{}%", props.help_progress))
                                                .dim(),
                                            Constraint::Min(4),
                                        ),
                                    ]
                                    .to_vec(),
                                )
                                .focus(props.focus)
                                .to_boxed()
                        })
                        .to_boxed(),
                )
                .to_boxed(),
            shortcuts: Shortcuts::new(state, action_tx.clone()).to_boxed(),
        }
    }

    fn handle_event(&mut self, key: termion::event::Key) {
        match key {
            Key::Esc | Key::Ctrl('c') => {
                let _ = self.base.action_tx.send(Action::Exit { selection: None });
            }
            Key::Char('?') => {
                let _ = self.base.action_tx.send(Action::LeavePage);
            }
            _ => {
                self.content.handle_event(key);
            }
        }
    }

    fn update(&mut self, state: &State) {
        self.props = HelpPageProps::from_callback(self.base.on_update, state)
            .unwrap_or(HelpPageProps::from(state));

        self.content.update(state);
    }

    fn render(&self, frame: &mut ratatui::Frame, area: Rect, props: Option<&dyn Any>) {
        let props = props
            .and_then(|props| props.downcast_ref::<HelpPageProps>())
            .unwrap_or(&self.props);

        let page_size = area.height.saturating_sub(6) as usize;

        let [content_area, shortcuts_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);

        self.content.render(frame, content_area, None);
        self.shortcuts.render(
            frame,
            shortcuts_area,
            Some(&ShortcutsProps::default().shortcuts(&props.shortcuts)),
        );

        if page_size != props.page_size {
            let _ = self.base.action_tx.send(Action::HelpPageSize(page_size));
        }
    }

    fn base_mut(&mut self) -> &mut BaseView<State, Action> {
        &mut self.base
    }
}

fn browse_footer<'a>(props: &BrowsePageProps<'a>, selected: Option<usize>) -> Vec<Column<'a>> {
    let filter = PatchItemFilter::from_str(&props.search).unwrap_or_default();

    let search = Line::from(vec![
        span::default(" Search ").cyan().dim().reversed(),
        span::default(" "),
        span::default(&props.search.to_string()).gray().dim(),
    ]);

    let draft = Line::from(vec![
        span::default(&props.stats.get("Draft").unwrap_or(&0).to_string()).dim(),
        span::default(" Draft").dim(),
    ]);

    let open = Line::from(vec![
        span::positive(&props.stats.get("Open").unwrap_or(&0).to_string()).dim(),
        span::default(" Open").dim(),
    ]);

    let merged = Line::from(vec![
        span::default(&props.stats.get("Merged").unwrap_or(&0).to_string())
            .magenta()
            .dim(),
        span::default(" Merged").dim(),
    ]);

    let archived = Line::from(vec![
        span::default(&props.stats.get("Archived").unwrap_or(&0).to_string())
            .yellow()
            .dim(),
        span::default(" Archived").dim(),
    ]);

    let len = props.patches.list().collect::<Vec<_>>().len();

    let sum = Line::from(vec![
        span::default("Σ ").dim(),
        span::default(&len.to_string()).dim(),
    ]);

    let progress = selected
        .map(|selected| TableUtils::progress(selected, len, props.page_size))
        .unwrap_or_default();
    let progress = span::default(&format!("{}%", progress)).dim();

    match filter.status() {
        Some(state) => {
            let block = match state {
                Status::Draft => draft,
                Status::Open => open,
                Status::Merged => merged,
                Status::Archived => archived,
            };

            vec![
                Column::new(Text::from(search), Constraint::Fill(1)),
                Column::new(
                    Text::from(block.clone()),
                    Constraint::Min(block.width() as u16),
                ),
                Column::new(Text::from(progress), Constraint::Min(4)),
            ]
        }
        None => vec![
            Column::new(Text::from(search), Constraint::Fill(1)),
            Column::new(
                Text::from(draft.clone()),
                Constraint::Min(draft.width() as u16),
            ),
            Column::new(
                Text::from(open.clone()),
                Constraint::Min(open.width() as u16),
            ),
            Column::new(
                Text::from(merged.clone()),
                Constraint::Min(merged.width() as u16),
            ),
            Column::new(
                Text::from(archived.clone()),
                Constraint::Min(archived.width() as u16),
            ),
            Column::new(Text::from(sum.clone()), Constraint::Min(sum.width() as u16)),
            Column::new(Text::from(progress), Constraint::Min(4)),
        ],
    }
}

fn help_text() -> Text<'static> {
    Text::from(
        [
            Line::from(Span::raw("Generic keybindings").cyan()),
            Line::raw(""),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "↑,k")).gray(),
                Span::raw(" "),
                Span::raw("move cursor one line up").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "↓,j")).gray(),
                Span::raw(" "),
                Span::raw("move cursor one line down").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "PageUp")).gray(),
                Span::raw(" "),
                Span::raw("move cursor one page up").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "PageDown")).gray(),
                Span::raw(" "),
                Span::raw("move cursor one page down").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "Home")).gray(),
                Span::raw(" "),
                Span::raw("move cursor to the first line").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "End")).gray(),
                Span::raw(" "),
                Span::raw("move cursor to the last line").gray().dim(),
            ]),
            Line::raw(""),
            Line::from(Span::raw("Specific keybindings").cyan()),
            Line::raw(""),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "enter")).gray(),
                Span::raw(" "),
                Span::raw("Select patch (if --mode id)").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "enter")).gray(),
                Span::raw(" "),
                Span::raw("Show patch").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "c")).gray(),
                Span::raw(" "),
                Span::raw("Checkout patch").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "d")).gray(),
                Span::raw(" "),
                Span::raw("Show patch diff").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "/")).gray(),
                Span::raw(" "),
                Span::raw("Search").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "?")).gray(),
                Span::raw(" "),
                Span::raw("Show help").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "Esc")).gray(),
                Span::raw(" "),
                Span::raw("Quit / cancel").gray().dim(),
            ]),
            Line::raw(""),
            Line::from(Span::raw("Searching").cyan()),
            Line::raw(""),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "Pattern")).gray(),
                Span::raw(" "),
                Span::raw("is:<state> | is:authored | authors:[<did>, <did>] | <search>")
                    .gray()
                    .dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "Example")).gray(),
                Span::raw(" "),
                Span::raw("is:open is:authored improve").gray().dim(),
            ]),
            Line::raw(""),
            Line::raw(""),
        ]
        .to_vec(),
    )
}
