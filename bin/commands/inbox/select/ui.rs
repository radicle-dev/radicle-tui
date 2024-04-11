use std::collections::HashMap;
use std::str::FromStr;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};

use radicle_tui as tui;

use tui::ui::items::{Filter, NotificationItem, NotificationItemFilter, NotificationState};
use tui::ui::span;
use tui::ui::widget::container::{Footer, Header};
use tui::ui::widget::input::TextField;
use tui::ui::widget::text::Paragraph;
use tui::ui::widget::{Column, Render, Shortcuts, Table, Widget};
use tui::Selection;

use crate::tui_inbox::common::{InboxOperation, Mode, RepositoryMode, SelectionMode};

use super::{Action, State};

pub struct ListPageProps {
    show_search: bool,
    show_help: bool,
}

impl From<&State> for ListPageProps {
    fn from(state: &State) -> Self {
        Self {
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
    notifications: Notifications<'a>,
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
            shortcuts: Shortcuts::new(&(), action_tx.clone()),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        let shorts = if state.ui.show_search {
            vec![("esc", "cancel"), ("enter", "apply")]
        } else if state.ui.show_help {
            vec![("?", "close")]
        } else {
            match state.mode.selection() {
                SelectionMode::Id => vec![("enter", "select"), ("/", "search")],
                SelectionMode::Operation => vec![
                    ("enter", "show"),
                    ("c", "clear"),
                    ("/", "search"),
                    ("?", "help"),
                ],
            }
        };

        let shortcuts = self.shortcuts.move_with_state(&());
        let shortcuts = shortcuts.shortcuts(&shorts);

        ListPage {
            notifications: self.notifications.move_with_state(state),
            shortcuts,
            help: self.help.move_with_state(state),
            props: ListPageProps::from(state),
            ..self
        }
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
        let _ = self.action_tx.send(Action::Update);
    }
}

impl<'a> Render<()> for ListPage<'a> {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, _area: Rect, _props: ()) {
        let area = frame.size();
        let layout = tui::ui::layout::default_page(area, 0u16, 1u16);

        if self.props.show_search {
            let component_layout = Layout::vertical([Constraint::Min(1), Constraint::Length(2)])
                .split(layout.component);

            self.notifications
                .render::<B>(frame, component_layout[0], ());
            self.search.render::<B>(frame, component_layout[1], ());
        } else if self.props.show_help {
            self.help.render::<B>(frame, layout.component, ());
        } else {
            self.notifications.render::<B>(frame, layout.component, ());
        }

        self.shortcuts.render::<B>(frame, layout.shortcuts, ());
    }
}

struct NotificationsProps<'a> {
    notifications: Vec<NotificationItem>,
    mode: Mode,
    stats: HashMap<String, usize>,
    columns: Vec<Column<'a>>,
    cutoff: usize,
    cutoff_after: usize,
    focus: bool,
    page_size: usize,
    search: String,
    show_search: bool,
}

impl<'a> From<&State> for NotificationsProps<'a> {
    fn from(state: &State) -> Self {
        let mut seen = 0;
        let mut unseen = 0;

        let notifications: Vec<NotificationItem> = state
            .notifications
            .iter()
            .filter(|issue| state.filter.matches(issue))
            .cloned()
            .collect();

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
            focus: false,
            page_size: state.ui.page_size,
            show_search: state.ui.show_search,
            search: state.search.read(),
        }
    }
}

struct Notifications<'a> {
    /// Action sender
    action_tx: UnboundedSender<Action>,
    /// State mapped props
    props: NotificationsProps<'a>,
    /// Notification table
    table: Table<'a, Action, NotificationItem>,
    /// Table footer
    footer: Footer<'a, Action>,
}

impl<'a> Widget<State, Action> for Notifications<'a> {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self {
        let props = NotificationsProps::from(state);
        let name = match state.mode.repository() {
            RepositoryMode::Contextual => state.project.name().to_string(),
            RepositoryMode::All => "All repositories".to_string(),
            RepositoryMode::ByRepo((_, name)) => name.clone().unwrap_or_default(),
        };

        Self {
            action_tx: action_tx.clone(),
            props: NotificationsProps::from(state),
            table: Table::new(&(), action_tx.clone())
                .items(props.notifications.clone())
                .columns(props.columns.to_vec())
                .header(
                    Header::new(&(), action_tx.clone())
                        .columns(
                            [
                                Column::new("", Constraint::Length(0)),
                                Column::new(Text::from(name), Constraint::Fill(1)),
                            ]
                            .to_vec(),
                        )
                        .cutoff(props.cutoff, props.cutoff_after)
                        .focus(props.focus),
                )
                .footer(!props.show_search)
                .cutoff(props.cutoff, props.cutoff_after),
            footer: Footer::new(&(), action_tx),
        }
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        let notifications: Vec<NotificationItem> = state
            .notifications
            .iter()
            .filter(|issue| state.filter.matches(issue))
            .cloned()
            .collect();

        let props = NotificationsProps::from(state);

        let table = self.table.move_with_state(&());
        let table = table
            .items(notifications)
            .footer(!state.ui.show_search)
            .page_size(state.ui.page_size);

        let footer = self.footer.move_with_state(&());
        let footer = footer.columns(Self::build_footer(&props, table.selected()));

        Self {
            props,
            table,
            footer,
            ..self
        }
    }

    fn handle_key_event(&mut self, key: Key) {
        match key {
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
            _ => {
                <Table<Action, NotificationItem> as Widget<(), Action>>::handle_key_event(
                    &mut self.table,
                    key,
                );
            }
        }
    }
}

impl<'a> Notifications<'a> {
    fn build_footer(props: &NotificationsProps<'a>, selected: Option<usize>) -> Vec<Column<'a>> {
        let search = Line::from(
            [
                span::default(" Search ".to_string())
                    .cyan()
                    .dim()
                    .reversed(),
                span::default(" ".into()),
                span::default(props.search.to_string()).gray().dim(),
            ]
            .to_vec(),
        );

        let seen = Line::from(
            [
                span::positive(props.stats.get("Seen").unwrap_or(&0).to_string()).dim(),
                span::default(" Seen".to_string()).dim(),
            ]
            .to_vec(),
        );
        let unseen = Line::from(
            [
                span::positive(props.stats.get("Unseen").unwrap_or(&0).to_string())
                    .magenta()
                    .dim(),
                span::default(" Unseen".to_string()).dim(),
            ]
            .to_vec(),
        );

        let progress = selected
            .map(|selected| {
                Table::<Action, NotificationItem>::progress(
                    selected,
                    props.notifications.len(),
                    props.page_size,
                )
            })
            .unwrap_or_default();
        let progress = span::default(format!("{}%", progress)).dim();

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
}

impl<'a> Render<()> for Notifications<'a> {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let header_height = 3_usize;

        let page_size = if self.props.show_search {
            self.table.render::<B>(frame, area, ());

            (area.height as usize).saturating_sub(header_height)
        } else {
            let layout = Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(area);

            self.table.render::<B>(frame, layout[0], ());
            self.footer.render::<B>(frame, layout[1], ());

            (area.height as usize).saturating_sub(header_height)
        };

        if page_size != self.props.page_size {
            let _ = self.action_tx.send(Action::PageSize(page_size));
        }
    }
}

pub struct Search {
    pub action_tx: UnboundedSender<Action>,
    pub input: TextField<Action>,
}

impl Widget<State, Action> for Search {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let input = TextField::new(state, action_tx.clone())
            .title("Search")
            .inline(true);
        Self { action_tx, input }.move_with_state(state)
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        let input = self.input.move_with_state(state);
        let input = input.text(&state.search.read().to_string());

        Self { input, ..self }
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
                <TextField<Action> as Widget<State, Action>>::handle_key_event(
                    &mut self.input,
                    key,
                );
                let _ = self.action_tx.send(Action::UpdateSearch {
                    value: self.input.read().to_string(),
                });
            }
        }
    }
}

impl Render<()> for Search {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let layout = Layout::horizontal(Constraint::from_mins([0]))
            .horizontal_margin(1)
            .split(area);

        self.input.render::<B>(frame, layout[0], ());
    }
}

#[derive(Clone)]
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
    header: Header<'a, Action>,
    /// Content widget
    content: Paragraph<'a, Action>,
    /// Container footer
    footer: Footer<'a, Action>,
}

impl<'a> Widget<State, Action> for Help<'a> {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let props = HelpProps::from(state);

        Self {
            action_tx: action_tx.clone(),
            props: props.clone(),
            header: Header::new(&(), action_tx.clone()),
            content: Paragraph::new(state, action_tx.clone()),
            footer: Footer::new(&(), action_tx),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        let props = HelpProps::from(state);

        let header = self.header.move_with_state(&());
        let header = header
            .columns([Column::new(" Help ", Constraint::Fill(1))].to_vec())
            .focus(props.focus);

        let content = self.content.move_with_state(state);
        let content = content.text(&props.content).page_size(props.page_size);

        let progress = span::default(format!("{}%", content.progress())).dim();

        let footer = self.footer.move_with_state(&());
        let footer = footer
            .columns(
                [
                    Column::new(Text::raw(""), Constraint::Fill(1)),
                    Column::new(Text::from(progress), Constraint::Min(4)),
                ]
                .to_vec(),
            )
            .focus(props.focus);

        Self {
            props,
            header,
            content,
            footer,
            ..self
        }
    }

    fn handle_key_event(&mut self, key: termion::event::Key) {
        match key {
            Key::Esc => {
                let _ = self.action_tx.send(Action::Exit { selection: None });
            }
            Key::Char('?') => {
                let _ = self.action_tx.send(Action::CloseHelp);
            }
            _ => {
                <Paragraph<_> as Widget<(), _>>::handle_key_event(&mut self.content, key);
            }
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

        self.header.render::<B>(frame, header_area, ());
        self.content.render::<B>(frame, content_area, ());
        self.footer.render::<B>(frame, footer_area, ());

        let page_size = content_area.height as usize;
        if page_size != self.props.page_size {
            let _ = self.action_tx.send(Action::PageSize(page_size));
        }
    }
}
