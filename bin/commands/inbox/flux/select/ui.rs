use std::collections::HashMap;
use std::str::FromStr;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;

use radicle::identity::Project;

use radicle_tui as tui;

use tui::flux::ui::items::{NotificationItem, NotificationItemFilter, NotificationState};
use tui::flux::ui::span;
use tui::flux::ui::widget::container::{Footer, FooterProps, Header, HeaderProps};
use tui::flux::ui::widget::input::{TextField, TextFieldProps};
use tui::flux::ui::widget::{
    Render, Shortcut, Shortcuts, ShortcutsProps, Table, TableProps, Widget,
};
use tui::Selection;

use crate::tui_inbox::common::{Mode, RepositoryMode, SelectionMode};

use super::{Action, State};

pub struct ListPageProps {
    selected: Option<NotificationItem>,
    mode: Mode,
    show_search: bool,
}

impl From<&State> for ListPageProps {
    fn from(state: &State) -> Self {
        Self {
            selected: state.selected.clone(),
            mode: state.mode.clone(),
            show_search: state.ui.show_search,
        }
    }
}

pub struct ListPage {
    /// Action sender
    pub action_tx: UnboundedSender<Action>,
    /// State mapped props
    props: ListPageProps,
    /// Notification widget
    notifications: Notifications,
    /// Search widget
    search: Search,
    /// Shortcut widget
    shortcuts: Shortcuts<Action>,
}

impl Widget<State, Action> for ListPage {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
            props: ListPageProps::from(state),
            notifications: Notifications::new(state, action_tx.clone()),
            search: Search::new(state, action_tx.clone()),
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
        } else {
            match key {
                Key::Esc | Key::Ctrl('c') => {
                    let _ = self.action_tx.send(Action::Exit { selection: None });
                }
                Key::Char('\n') => {
                    if let Some(selected) = &self.props.selected {
                        let selection = match self.props.mode.selection() {
                            SelectionMode::Operation => Selection::default()
                                .with_operation("show".to_string())
                                .with_id(selected.id),
                            SelectionMode::Id => Selection::default().with_id(selected.id),
                        };
                        let _ = self.action_tx.send(Action::Exit {
                            selection: Some(selection),
                        });
                    }
                }
                Key::Char('c') => {
                    if let Some(selected) = &self.props.selected {
                        let _ = self.action_tx.send(Action::Exit {
                            selection: Some(
                                Selection::default()
                                    .with_operation("clear".to_string())
                                    .with_id(selected.id),
                            ),
                        });
                    }
                }
                Key::Char('/') => {
                    let _ = self.action_tx.send(Action::OpenSearch);
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

impl Render<()> for ListPage {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, _area: Rect, _props: ()) {
        let area = frame.size();
        let layout = tui::flux::ui::layout::default_page(area, 0u16, 1u16);

        let shortcuts = if self.props.show_search {
            vec![
                Shortcut::new("esc", "back"),
                Shortcut::new("enter", "search"),
            ]
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
            _ => {}
        }
        self.table
            .selected()
            .and_then(|selected| self.props.notifications.get(selected))
            .and_then(|notif| {
                self.action_tx
                    .send(Action::Select {
                        item: notif.clone(),
                    })
                    .ok()
            });
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
        let search = if self.props.search.is_empty() {
            Line::from([span::default(self.props.search.to_string()).magenta().dim()].to_vec())
        } else {
            Line::from(
                [
                    span::default(" / ".to_string()).magenta().dim(),
                    span::default(self.props.search.to_string()).magenta().dim(),
                ]
                .to_vec(),
            )
        };

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
                titles: ("/".into(), "Search".into()),
                show_cursor: true,
                inline_label: true,
            },
        );
    }
}
