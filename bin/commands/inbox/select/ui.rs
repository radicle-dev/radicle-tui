use std::collections::HashMap;
use std::str::FromStr;

use ratatui::widgets::TableState;
use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::layout::{Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};

use radicle_tui as tui;

use tui::ui::items::{NotificationItem, NotificationItemFilter, NotificationState};
use tui::ui::span;
use tui::ui::widget::container::{
    Container, ContainerProps, Footer, FooterProps, Header, HeaderProps,
};
use tui::ui::widget::input::{TextField, TextFieldProps, TextFieldState};
use tui::ui::widget::text::{Paragraph, ParagraphProps, ParagraphState};
use tui::ui::widget::{self, BaseView, RenderProps, TableUtils, WidgetState};
use tui::ui::widget::{Column, Properties, Shortcuts, ShortcutsProps, Table, TableProps, Widget};
use tui::Selection;

use crate::tui_inbox::common::{InboxOperation, Mode, RepositoryMode, SelectionMode};

use super::{Action, State};

type BoxedWidget = widget::BoxedWidget<State, Action>;

#[derive(Clone)]
struct BrowsePageProps<'a> {
    notifications: Vec<NotificationItem>,
    selected: Option<usize>,
    mode: Mode,
    stats: HashMap<String, usize>,
    columns: Vec<Column<'a>>,
    cutoff: usize,
    cutoff_after: usize,
    page_size: usize,
    search: String,
    show_search: bool,
    shortcuts: Vec<(&'a str, &'a str)>,
}

impl<'a> From<&State> for BrowsePageProps<'a> {
    fn from(state: &State) -> Self {
        let mut seen = 0;
        let mut unseen = 0;

        let notifications = state.browser.notifications();

        // Compute statistics
        for notification in &notifications {
            if notification.seen {
                seen += 1;
            } else {
                unseen += 1;
            }
        }

        let stats = HashMap::from([("Seen".to_string(), seen), ("Unseen".to_string(), unseen)]);

        Self {
            notifications,
            mode: state.mode.clone(),
            stats,
            columns: [
                Column::new("", Constraint::Length(5)),
                Column::new("", Constraint::Length(3)),
                Column::new("", Constraint::Length(15))
                    .skip(*state.mode.repository() != RepositoryMode::All),
                Column::new("", Constraint::Length(25)),
                Column::new("", Constraint::Fill(1)),
                Column::new("", Constraint::Length(8)),
                Column::new("", Constraint::Length(10)),
                Column::new("", Constraint::Length(15)),
                Column::new("", Constraint::Length(18)),
            ]
            .to_vec(),
            cutoff: 200,
            cutoff_after: 5,
            search: state.browser.search.read(),
            page_size: state.browser.page_size,
            show_search: state.browser.show_search,
            selected: state.browser.selected,
            shortcuts: match state.mode.selection() {
                SelectionMode::Id => vec![("enter", "select"), ("/", "search")],
                SelectionMode::Operation => vec![
                    ("enter", "show"),
                    ("c", "clear"),
                    ("/", "search"),
                    ("?", "help"),
                ],
            },
        }
    }
}

impl<'a> Properties for BrowsePageProps<'a> {}

pub struct BrowsePage<'a> {
    /// Internal base
    base: BaseView<State, Action>,
    /// Internal props
    props: BrowsePageProps<'a>,
    /// Notifications widget
    notifications: BoxedWidget,
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
        let name = match state.mode.repository() {
            RepositoryMode::Contextual => state.project.name().to_string(),
            RepositoryMode::All => "All repositories".to_string(),
            RepositoryMode::ByRepo((_, name)) => name.clone().unwrap_or_default(),
        };

        Self {
            base: BaseView {
                action_tx: action_tx.clone(),
                on_update: None,
                on_event: None,
            },
            props: props.clone(),
            notifications: Container::new(state, action_tx.clone())
                .header(
                    Header::new(state, action_tx.clone())
                        .columns(
                            [
                                Column::new("", Constraint::Length(0)),
                                Column::new(Text::from(name), Constraint::Fill(1)),
                            ]
                            .to_vec(),
                        )
                        .cutoff(props.cutoff, props.cutoff_after)
                        .to_boxed(),
                )
                .content(Box::<Table<State, Action, NotificationItem, 9>>::new(
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
                                .items(state.browser.notifications())
                                .footer(!state.browser.show_search)
                                .page_size(state.browser.page_size)
                                .cutoff(props.cutoff, props.cutoff_after)
                                .to_boxed()
                        }),
                ))
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
                .on_update(|state| {
                    ContainerProps::default()
                        .hide_footer(BrowsePageProps::from(state).show_search)
                        .to_boxed()
                })
                .to_boxed(),
            search: Search::new(state, action_tx.clone()).to_boxed(),
            shortcuts: Shortcuts::new(state, action_tx.clone())
                .on_update(|state| {
                    ShortcutsProps::default()
                        .shortcuts(&BrowsePageProps::from(state).shortcuts)
                        .to_boxed()
                })
                .to_boxed(),
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
                    self.props
                        .selected
                        .and_then(|selected| self.props.notifications.get(selected))
                        .and_then(|notif| {
                            let selection = match self.props.mode.selection() {
                                SelectionMode::Operation => Selection::default()
                                    .with_operation(InboxOperation::Show.to_string())
                                    .with_id(notif.id),
                                SelectionMode::Id => Selection::default().with_id(notif.id),
                            };

                            self.base
                                .action_tx
                                .send(Action::Exit {
                                    selection: Some(selection),
                                })
                                .ok()
                        });
                }
                Key::Char('c') => {
                    self.props
                        .selected
                        .and_then(|selected| self.props.notifications.get(selected))
                        .and_then(|notif| {
                            self.base
                                .action_tx
                                .send(Action::Exit {
                                    selection: Some(
                                        Selection::default()
                                            .with_operation(InboxOperation::Clear.to_string())
                                            .with_id(notif.id),
                                    ),
                                })
                                .ok()
                        });
                }
                _ => {
                    self.notifications.handle_event(key);
                }
            }
        }
    }

    fn update(&mut self, state: &State) {
        self.props = BrowsePageProps::from_callback(self.base.on_update, state)
            .unwrap_or(BrowsePageProps::from(state));

        self.notifications.update(state);
        self.search.update(state);
        self.shortcuts.update(state);
    }

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let page_size = props.area.height.saturating_sub(6) as usize;

        let [content_area, shortcuts_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(props.area);

        if self.props.show_search {
            let [table_area, search_area] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(content_area);

            self.notifications
                .render(frame, RenderProps::from(table_area));
            self.search
                .render(frame, RenderProps::from(search_area).focus(true));
        } else {
            self.notifications
                .render(frame, RenderProps::from(content_area).focus(true));
        }

        self.shortcuts
            .render(frame, RenderProps::from(shortcuts_area));

        if page_size != self.props.page_size {
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

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let layout = Layout::horizontal(Constraint::from_mins([0]))
            .horizontal_margin(1)
            .split(props.area);

        self.input.render(frame, RenderProps::from(layout[0]));
    }

    fn base_mut(&mut self) -> &mut BaseView<State, Action> {
        &mut self.base
    }
}

#[derive(Clone)]
struct HelpPageProps<'a> {
    page_size: usize,
    help_progress: usize,
    shortcuts: Vec<(&'a str, &'a str)>,
}

impl<'a> From<&State> for HelpPageProps<'a> {
    fn from(state: &State) -> Self {
        Self {
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
                        .on_update(|_| {
                            HeaderProps::default()
                                .columns([Column::new(" Help ", Constraint::Fill(1))].to_vec())
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
                                .to_boxed()
                        })
                        .to_boxed(),
                )
                .to_boxed(),
            shortcuts: Shortcuts::new(state, action_tx.clone())
                .on_update(|state| {
                    ShortcutsProps::default()
                        .shortcuts(&HelpPageProps::from(state).shortcuts)
                        .to_boxed()
                })
                .to_boxed(),
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
        self.shortcuts.update(state);
    }

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let page_size = props.area.height.saturating_sub(6) as usize;

        let [content_area, shortcuts_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(props.area);

        self.content
            .render(frame, RenderProps::from(content_area).focus(true));
        self.shortcuts
            .render(frame, RenderProps::from(shortcuts_area));

        if page_size != self.props.page_size {
            let _ = self.base.action_tx.send(Action::HelpPageSize(page_size));
        }
    }

    fn base_mut(&mut self) -> &mut BaseView<State, Action> {
        &mut self.base
    }
}

fn browse_footer<'a>(props: &BrowsePageProps<'a>, selected: Option<usize>) -> Vec<Column<'a>> {
    let search = Line::from(vec![
        span::default(" Search ").cyan().dim().reversed(),
        span::default(" "),
        span::default(&props.search.to_string()).gray().dim(),
    ]);

    let seen = Line::from(vec![
        span::positive(&props.stats.get("Seen").unwrap_or(&0).to_string()).dim(),
        span::default(" Seen").dim(),
    ]);
    let unseen = Line::from(vec![
        span::positive(&props.stats.get("Unseen").unwrap_or(&0).to_string())
            .magenta()
            .dim(),
        span::default(" Unseen").dim(),
    ]);

    let progress = selected
        .map(|selected| TableUtils::progress(selected, props.notifications.len(), props.page_size))
        .unwrap_or_default();
    let progress = span::default(&format!("{}%", progress)).dim();

    match NotificationItemFilter::from_str(&props.search)
        .unwrap_or_default()
        .state()
    {
        Some(state) => {
            let block = match state {
                NotificationState::Seen => seen,
                NotificationState::Unseen => unseen,
            };

            [
                Column::new(Text::from(search), Constraint::Fill(1)),
                Column::new(
                    Text::from(block.clone()),
                    Constraint::Min(block.width() as u16),
                ),
                Column::new(Text::from(progress), Constraint::Min(4)),
            ]
            .to_vec()
        }
        None => [
            Column::new(Text::from(search), Constraint::Fill(1)),
            Column::new(
                Text::from(seen.clone()),
                Constraint::Min(seen.width() as u16),
            ),
            Column::new(
                Text::from(unseen.clone()),
                Constraint::Min(unseen.width() as u16),
            ),
            Column::new(Text::from(progress), Constraint::Min(4)),
        ]
        .to_vec(),
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
                Span::raw("Select notification (if --mode id)").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "enter")).gray(),
                Span::raw(" "),
                Span::raw("Show notification").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "c")).gray(),
                Span::raw(" "),
                Span::raw("Clear notifications").gray().dim(),
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
                Span::raw("is:<state> | is:patch | is:issue | <search>")
                    .gray()
                    .dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "Example")).gray(),
                Span::raw(" "),
                Span::raw("is:unseen is:patch Print").gray().dim(),
            ]),
            Line::raw(""),
            Line::raw(""),
        ]
        .to_vec(),
    )
}
