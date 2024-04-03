use std::collections::HashMap;
use std::str::FromStr;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};

use radicle::identity::Project;

use radicle_tui as tui;

use tui::flux::ui::items::{NotificationItem, NotificationItemFilter, NotificationState};
use tui::flux::ui::span;
use tui::flux::ui::widget::container::{Footer, FooterProps, Header, HeaderProps};
use tui::flux::ui::widget::input::{TextField, TextFieldProps};
use tui::flux::ui::widget::text::{Paragraph, ParagraphProps};
use tui::flux::ui::widget::{
    Render, Shortcut, Shortcuts, ShortcutsProps, Table, TableProps, Widget,
};
use tui::Selection;

use crate::tui_inbox::common::{InboxOperation, Mode, RepositoryMode, SelectionMode};

use super::{Action, State};

pub struct ListPageProps {
    mode: Mode,
    show_search: bool,
    show_help: bool,
}

impl From<&State> for ListPageProps {
    fn from(state: &State) -> Self {
        Self {
            mode: state.mode.clone(),
            show_search: state.ui.show_search,
            show_help: state.ui.show_help,
        }
    }
}

pub struct ListPage<'a> {
    /// Action sender
    pub action_tx: UnboundedSender<Action>,
    /// State mapped props
    props: ListPageProps,
    /// Notification widget
    notifications: Notifications,
    /// Search widget
    search: Search,
    /// Help widget
    help: Help<'a>,
    /// Shortcut widget
    shortcuts: Shortcuts<Action>,
}

impl<'a> Widget<State, Action> for ListPage<'a> {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
            props: ListPageProps::from(state),
            notifications: Notifications::new(state, action_tx.clone()),
            search: Search::new(state, action_tx.clone()),
            help: Help::new(state, action_tx.clone()),
            shortcuts: Shortcuts::new(state, action_tx.clone()),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        ListPage {
            notifications: self.notifications.move_with_state(state),
            shortcuts: self.shortcuts.move_with_state(state),
            help: self.help.move_with_state(state),
            props: ListPageProps::from(state),
            ..self
        }
    }

    fn name(&self) -> &str {
        "list-page"
    }

    fn handle_key_event(&mut self, key: termion::event::Key) {
        if self.props.show_search {
            <Search as Widget<State, Action>>::handle_key_event(&mut self.search, key)
        } else if self.props.show_help {
            <Help as Widget<State, Action>>::handle_key_event(&mut self.help, key)
        } else {
            match key {
                Key::Esc | Key::Ctrl('c') => {
                    let _ = self.action_tx.send(Action::Exit { selection: None });
                }
                Key::Char('/') => {
                    let _ = self.action_tx.send(Action::OpenSearch);
                }
                Key::Char('?') => {
                    let _ = self.action_tx.send(Action::OpenHelp);
                }
                _ => {
                    <Notifications as Widget<State, Action>>::handle_key_event(
                        &mut self.notifications,
                        key,
                    );
                }
            }
        }
    }
}

impl<'a> Render<()> for ListPage<'a> {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, _area: Rect, _props: ()) {
        let area = frame.size();
        let layout = tui::flux::ui::layout::default_page(area, 0u16, 1u16);

        let shortcuts = if self.props.show_search {
            vec![
                Shortcut::new("esc", "cancel"),
                Shortcut::new("enter", "apply"),
            ]
        } else if self.props.show_help {
            vec![Shortcut::new("?", "close")]
        } else {
            match self.props.mode.selection() {
                SelectionMode::Id => vec![
                    Shortcut::new("enter", "select"),
                    Shortcut::new("/", "search"),
                ],
                SelectionMode::Operation => vec![
                    Shortcut::new("enter", "show"),
                    Shortcut::new("c", "clear"),
                    Shortcut::new("/", "search"),
                    Shortcut::new("?", "help"),
                ],
            }
        };

        if self.props.show_search {
            let component_layout = Layout::vertical([Constraint::Min(1), Constraint::Length(2)])
                .split(layout.component);

            self.notifications
                .render::<B>(frame, component_layout[0], ());
            self.search
                .render::<B>(frame, component_layout[1], SearchProps {});
        } else if self.props.show_help {
            self.help.render::<B>(frame, layout.component, ());
        } else {
            self.notifications.render::<B>(frame, layout.component, ());
        }

        self.shortcuts.render::<B>(
            frame,
            layout.shortcuts,
            ShortcutsProps {
                shortcuts,
                divider: '∙',
            },
        );
    }
}

struct NotificationsProps {
    notifications: Vec<NotificationItem>,
    mode: Mode,
    project: Project,
    stats: HashMap<String, usize>,
    cutoff: usize,
    cutoff_after: usize,
    focus: bool,
    page_size: usize,
    search: String,
    show_search: bool,
}

impl From<&State> for NotificationsProps {
    fn from(state: &State) -> Self {
        let mut seen = 0;
        let mut unseen = 0;

        // Filter by search string
        let filter = NotificationItemFilter::from_str(&state.search.read()).unwrap_or_default();
        let notifications = state
            .notifications
            .clone()
            .into_iter()
            .filter(|issue| filter.matches(issue))
            .collect::<Vec<_>>();

        // Compute statistics
        for notification in &state.notifications {
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
            project: state.project.clone(),
            stats,
            cutoff: 200,
            cutoff_after: 5,
            focus: false,
            page_size: state.ui.page_size,
            show_search: state.ui.show_search,
            search: state.search.read(),
        }
    }
}

struct Notifications {
    /// Action sender
    action_tx: UnboundedSender<Action>,
    /// State mapped props
    props: NotificationsProps,
    /// Table header
    header: Header<Action>,
    /// Notification table
    table: Table<Action>,
    /// Table footer
    footer: Footer<Action>,
}

impl Widget<State, Action> for Notifications {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self {
        Self {
            action_tx: action_tx.clone(),
            props: NotificationsProps::from(state),
            header: Header::new(state, action_tx.clone()),
            table: Table::new(state, action_tx.clone()),
            footer: Footer::new(state, action_tx),
        }
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        let props = NotificationsProps::from(state);
        let mut table = self.table.move_with_state(state);

        if let Some(selected) = table.selected() {
            if selected > props.notifications.len() {
                table.begin();
            }
        }

        Self {
            props,
            table,
            header: self.header.move_with_state(state),
            footer: self.footer.move_with_state(state),
            ..self
        }
    }

    fn name(&self) -> &str {
        "notifications"
    }

    fn handle_key_event(&mut self, key: Key) {
        match key {
            Key::Up | Key::Char('k') => {
                self.table.prev();
            }
            Key::Down | Key::Char('j') => {
                self.table.next(self.props.notifications.len());
            }
            Key::PageUp => {
                self.table.prev_page(self.props.page_size);
            }
            Key::PageDown => {
                self.table
                    .next_page(self.props.notifications.len(), self.props.page_size);
            }
            Key::Home => {
                self.table.begin();
            }
            Key::End => {
                self.table.end(self.props.notifications.len());
            }
            Key::Char('\n') => {
                self.table
                    .selected()
                    .and_then(|selected| self.props.notifications.get(selected))
                    .and_then(|notif| {
                        let selection = match self.props.mode.selection() {
                            SelectionMode::Operation => Selection::default()
                                .with_operation(InboxOperation::Show.to_string())
                                .with_id(notif.id),
                            SelectionMode::Id => Selection::default().with_id(notif.id),
                        };

                        self.action_tx
                            .send(Action::Exit {
                                selection: Some(selection),
                            })
                            .ok()
                    });
            }
            Key::Char('c') => {
                self.table
                    .selected()
                    .and_then(|selected| self.props.notifications.get(selected))
                    .and_then(|notif| {
                        self.action_tx
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
            _ => {}
        }
    }
}

impl Notifications {
    fn render_header<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect) {
        let title = match self.props.mode.repository() {
            RepositoryMode::Contextual => self.props.project.name().to_string(),
            RepositoryMode::All => "All repositories".to_string(),
            RepositoryMode::ByRepo((_, name)) => name.clone().unwrap_or_default(),
        };

        self.header.render::<B>(
            frame,
            area,
            HeaderProps {
                cells: [String::from("").into(), title.into()],
                widths: [Constraint::Length(0), Constraint::Fill(1)],
                focus: self.props.focus,
                cutoff: self.props.cutoff,
                cutoff_after: self.props.cutoff_after,
            },
        );
    }

    fn render_list<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect) {
        if let RepositoryMode::All = self.props.mode.repository() {
            let widths = [
                Constraint::Length(5),
                Constraint::Length(3),
                Constraint::Length(15),
                Constraint::Length(25),
                Constraint::Fill(1),
                Constraint::Length(8),
                Constraint::Length(10),
                Constraint::Length(15),
                Constraint::Length(18),
            ];

            self.table.render::<B>(
                frame,
                area,
                TableProps {
                    items: self.props.notifications.to_vec(),
                    has_header: true,
                    has_footer: !self.props.show_search,
                    widths,
                    focus: self.props.focus,
                    cutoff: self.props.cutoff,
                    cutoff_after: self.props.cutoff_after.saturating_add(1),
                },
            );
        } else {
            let widths = [
                Constraint::Length(5),
                Constraint::Length(3),
                Constraint::Length(25),
                Constraint::Fill(1),
                Constraint::Length(8),
                Constraint::Length(10),
                Constraint::Length(15),
                Constraint::Length(18),
            ];

            self.table.render::<B>(
                frame,
                area,
                TableProps {
                    items: self.props.notifications.to_vec(),
                    has_header: true,
                    has_footer: !self.props.show_search,
                    widths,
                    focus: self.props.focus,
                    cutoff: self.props.cutoff,
                    cutoff_after: self.props.cutoff_after,
                },
            );
        }
    }

    fn render_footer<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect) {
        let search = Line::from(
            [
                span::default(" Search ".to_string())
                    .cyan()
                    .dim()
                    .reversed(),
                span::default(" ".into()),
                span::default(self.props.search.to_string()).gray().dim(),
            ]
            .to_vec(),
        );

        let seen = Line::from(
            [
                span::positive(self.props.stats.get("Seen").unwrap_or(&0).to_string()).dim(),
                span::default(" Seen".to_string()).dim(),
            ]
            .to_vec(),
        );
        let unseen = Line::from(
            [
                span::positive(self.props.stats.get("Unseen").unwrap_or(&0).to_string())
                    .magenta()
                    .dim(),
                span::default(" Unseen".to_string()).dim(),
            ]
            .to_vec(),
        );

        let progress = self
            .table
            .progress_percentage(self.props.notifications.len(), self.props.page_size);
        let progress = span::default(format!("{}%", progress)).dim();

        match NotificationItemFilter::from_str(&self.props.search)
            .unwrap_or_default()
            .state()
        {
            Some(state) => {
                let block = match state {
                    NotificationState::Seen => seen,
                    NotificationState::Unseen => unseen,
                };

                self.footer.render::<B>(
                    frame,
                    area,
                    FooterProps {
                        cells: [search.into(), block.clone().into(), progress.clone().into()],
                        widths: [
                            Constraint::Fill(1),
                            Constraint::Min(block.width() as u16),
                            Constraint::Min(4),
                        ],
                        focus: self.props.focus,
                        cutoff: self.props.cutoff,
                        cutoff_after: self.props.cutoff_after,
                    },
                );
            }
            None => {
                self.footer.render::<B>(
                    frame,
                    area,
                    FooterProps {
                        cells: [
                            search.into(),
                            seen.clone().into(),
                            unseen.clone().into(),
                            progress.clone().into(),
                        ],
                        widths: [
                            Constraint::Fill(1),
                            Constraint::Min(seen.width() as u16),
                            Constraint::Min(unseen.width() as u16),
                            Constraint::Min(4),
                        ],
                        focus: self.props.focus,
                        cutoff: self.props.cutoff,
                        cutoff_after: self.props.cutoff_after,
                    },
                );
            }
        }
    }
}

impl Render<()> for Notifications {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let page_size = if self.props.show_search {
            let layout = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(area);

            self.render_header::<B>(frame, layout[0]);
            self.render_list::<B>(frame, layout[1]);

            layout[1].height as usize
        } else {
            let layout = Layout::vertical([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(area);

            self.render_header::<B>(frame, layout[0]);
            self.render_list::<B>(frame, layout[1]);
            self.render_footer::<B>(frame, layout[2]);

            layout[1].height as usize
        };

        if page_size != self.props.page_size {
            let _ = self.action_tx.send(Action::PageSize(page_size));
        }
    }
}

pub struct SearchProps {}

pub struct Search {
    pub action_tx: UnboundedSender<Action>,
    pub input: TextField,
}

impl Widget<State, Action> for Search {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let mut input = TextField::new(state, action_tx.clone());
        input.set_text(&state.search.read().to_string());

        Self { action_tx, input }.move_with_state(state)
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        let mut input = <TextField as Widget<State, Action>>::move_with_state(self.input, state);
        input.set_text(&state.search.read().to_string());

        Self { input, ..self }
    }

    fn name(&self) -> &str {
        "filter-popup"
    }

    fn handle_key_event(&mut self, key: termion::event::Key) {
        match key {
            Key::Esc => {
                let _ = self.action_tx.send(Action::CloseSearch);
            }
            Key::Char('\n') => {
                let _ = self.action_tx.send(Action::ApplySearch);
            }
            _ => {
                <TextField as Widget<State, Action>>::handle_key_event(&mut self.input, key);
                let _ = self.action_tx.send(Action::UpdateSearch {
                    value: self.input.text().to_string(),
                });
            }
        }
    }
}

impl Render<SearchProps> for Search {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, _props: SearchProps) {
        let layout = Layout::horizontal(Constraint::from_mins([0]))
            .horizontal_margin(1)
            .split(area);

        self.input.render::<B>(
            frame,
            layout[0],
            TextFieldProps {
                titles: ("Search".into(), "Search".into()),
                show_cursor: true,
                inline_label: true,
            },
        );
    }
}

pub struct HelpProps<'a> {
    content: Text<'a>,
    focus: bool,
    page_size: usize,
}

impl<'a> From<&State> for HelpProps<'a> {
    fn from(state: &State) -> Self {
        let content = Text::from(
            [
                Line::from(Span::raw("Generic keybindings").cyan()),
                Line::raw(""),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "↑,k")).gray(),
                        Span::raw(" "),
                        Span::raw("move cursor one line up").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "↓,j")).gray(),
                        Span::raw(" "),
                        Span::raw("move cursor one line down").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "PageUp")).gray(),
                        Span::raw(" "),
                        Span::raw("move cursor one page up").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "PageDown")).gray(),
                        Span::raw(" "),
                        Span::raw("move cursor one page down").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "Home")).gray(),
                        Span::raw(" "),
                        Span::raw("move cursor to the first line").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "End")).gray(),
                        Span::raw(" "),
                        Span::raw("move cursor to the last line").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::raw(""),
                Line::from(Span::raw("Specific keybindings").cyan()),
                Line::raw(""),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "enter")).gray(),
                        Span::raw(" "),
                        Span::raw("Select notification (if --mode id)").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "enter")).gray(),
                        Span::raw(" "),
                        Span::raw("Show notification").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "c")).gray(),
                        Span::raw(" "),
                        Span::raw("Clear notifications").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "/")).gray(),
                        Span::raw(" "),
                        Span::raw("Search").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "?")).gray(),
                        Span::raw(" "),
                        Span::raw("Show help").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "Esc")).gray(),
                        Span::raw(" "),
                        Span::raw("Quit / cancel").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::raw(""),
                Line::from(Span::raw("Searching").cyan()),
                Line::raw(""),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "Pattern")).gray(),
                        Span::raw(" "),
                        Span::raw("is:<state> | is:patch | is:issue | <search>")
                            .gray()
                            .dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "Example")).gray(),
                        Span::raw(" "),
                        Span::raw("is:unseen is:patch Print").gray().dim(),
                    ]
                    .to_vec(),
                ),
            ]
            .to_vec(),
        );

        Self {
            content,
            focus: false,
            page_size: state.ui.page_size,
        }
    }
}

pub struct Help<'a> {
    /// Send messages
    pub action_tx: UnboundedSender<Action>,
    /// This widget's render properties
    pub props: HelpProps<'a>,
    /// Container header
    header: Header<Action>,
    /// Content widget
    content: Paragraph<Action>,
    /// Container footer
    footer: Footer<Action>,
}

impl<'a> Widget<State, Action> for Help<'a> {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
            props: HelpProps::from(state),
            header: Header::new(state, action_tx.clone()),
            content: Paragraph::new(state, action_tx.clone()),
            footer: Footer::new(state, action_tx),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        Self {
            props: HelpProps::from(state),
            header: self.header.move_with_state(state),
            content: self.content.move_with_state(state),
            footer: self.footer.move_with_state(state),
            ..self
        }
    }

    fn name(&self) -> &str {
        "help"
    }

    fn handle_key_event(&mut self, key: termion::event::Key) {
        let len = self.props.content.lines.len() + 1;
        let page_size = self.props.page_size;
        match key {
            Key::Esc => {
                let _ = self.action_tx.send(Action::Exit { selection: None });
            }
            Key::Char('?') => {
                let _ = self.action_tx.send(Action::CloseHelp);
            }
            Key::Up | Key::Char('k') => {
                self.content.prev(len, page_size);
            }
            Key::Down | Key::Char('j') => {
                self.content.next(len, page_size);
            }
            Key::PageUp => {
                self.content.prev_page(len, page_size);
            }
            Key::PageDown => {
                self.content.next_page(len, page_size);
            }
            Key::Home => {
                self.content.begin(len, page_size);
            }
            Key::End => {
                self.content.end(len, page_size);
            }
            _ => {}
        }
    }
}

impl<'a> Render<()> for Help<'a> {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let [header_area, content_area, footer_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .areas(area);

        self.header.render::<B>(
            frame,
            header_area,
            HeaderProps {
                cells: [String::from(" Help ").into()],
                widths: [Constraint::Fill(1)],
                focus: self.props.focus,
                cutoff: usize::MIN,
                cutoff_after: usize::MAX,
            },
        );

        self.content.render::<B>(
            frame,
            content_area,
            ParagraphProps {
                content: self.props.content.clone(),
                focus: self.props.focus,
                has_footer: true,
                has_header: true,
            },
        );

        let progress = span::default(format!("{}%", self.content.progress())).dim();

        self.footer.render::<B>(
            frame,
            footer_area,
            FooterProps {
                cells: [String::new().into(), progress.clone().into()],
                widths: [Constraint::Fill(1), Constraint::Min(4)],
                focus: self.props.focus,
                cutoff: usize::MAX,
                cutoff_after: usize::MAX,
            },
        );

        let page_size = content_area.height as usize;
        if page_size != self.props.page_size {
            let _ = self.action_tx.send(Action::PageSize(page_size));
        }
    }
}
