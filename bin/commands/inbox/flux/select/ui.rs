use std::collections::HashMap;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;

use radicle::identity::Project;

use radicle_tui as tui;

use tui::flux::ui::cob::NotificationItem;
use tui::flux::ui::span;
use tui::flux::ui::widget::container::{Footer, FooterProps, Header, HeaderProps};
use tui::flux::ui::widget::{
    Render, Shortcut, Shortcuts, ShortcutsProps, Table, TableProps, Widget,
};
use tui::Selection;

use crate::tui_inbox::common::{Mode, RepositoryMode, SelectionMode};

use super::{Action, InboxState};

pub struct ListPageProps {
    selected: Option<NotificationItem>,
    mode: Mode,
}

impl From<&InboxState> for ListPageProps {
    fn from(state: &InboxState) -> Self {
        Self {
            selected: state.selected.clone(),
            mode: state.mode.clone(),
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
    /// Shortcut widget
    shortcuts: Shortcuts<Action>,
}

impl Widget<InboxState, Action> for ListPage {
    fn new(state: &InboxState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
            props: ListPageProps::from(state),
            notifications: Notifications::new(state, action_tx.clone()),
            shortcuts: Shortcuts::new(state, action_tx.clone()),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &InboxState) -> Self
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
            _ => {
                <Notifications as Widget<InboxState, Action>>::handle_key_event(
                    &mut self.notifications,
                    key,
                );
            }
        }
    }
}

impl Render<()> for ListPage {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, _area: Rect, _props: ()) {
        let area = frame.size();
        let layout = tui::flux::ui::layout::default_page(area, 0u16, 1u16);

        let shortcuts = match self.props.mode.selection() {
            SelectionMode::Id => vec![Shortcut::new("enter", "select")],
            SelectionMode::Operation => {
                vec![Shortcut::new("enter", "show"), Shortcut::new("c", "clear")]
            }
        };

        self.notifications.render::<B>(frame, layout.component, ());
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
}

impl From<&InboxState> for NotificationsProps {
    fn from(state: &InboxState) -> Self {
        let mut seen = 0;
        let mut unseen = 0;

        for notification in &state.notifications {
            if notification.seen {
                seen += 1;
            } else {
                unseen += 1;
            }
        }
        let stats = HashMap::from([("Seen".to_string(), seen), ("Unseen".to_string(), unseen)]);

        Self {
            notifications: state.notifications.clone(),
            mode: state.mode.clone(),
            project: state.project.clone(),
            stats,
            cutoff: 200,
            cutoff_after: 5,
            focus: false,
            page_size: state.ui.page_size,
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

impl Widget<InboxState, Action> for Notifications {
    fn new(state: &InboxState, action_tx: UnboundedSender<Action>) -> Self {
        Self {
            action_tx: action_tx.clone(),
            props: NotificationsProps::from(state),
            header: Header::new(state, action_tx.clone()),
            table: Table::new(state, action_tx.clone()),
            footer: Footer::new(state, action_tx),
        }
    }

    fn move_with_state(self, state: &InboxState) -> Self
    where
        Self: Sized,
    {
        Self {
            props: NotificationsProps::from(state),
            header: self.header.move_with_state(state),
            table: self.table.move_with_state(state),
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

                let selected = self
                    .table
                    .selected()
                    .and_then(|selected| self.props.notifications.get(selected));

                // TODO: propagate error
                if let Some(notif) = selected {
                    let _ = self.action_tx.send(Action::Select {
                        item: notif.clone(),
                    });
                }
            }
            Key::Down | Key::Char('j') => {
                self.table.next(self.props.notifications.len());

                let selected = self
                    .table
                    .selected()
                    .and_then(|selected| self.props.notifications.get(selected));

                // TODO: propagate error
                if let Some(notif) = selected {
                    let _ = self.action_tx.send(Action::Select {
                        item: notif.clone(),
                    });
                }
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
                    has_footer: true,
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
                    has_footer: true,
                    widths,
                    focus: self.props.focus,
                    cutoff: self.props.cutoff,
                    cutoff_after: self.props.cutoff_after,
                },
            );
        }
    }

    fn render_footer<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect) {
        let filter = Line::from([span::blank()].to_vec());
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
        let sum = Line::from(
            [
                span::default("Σ ".to_string()).dim(),
                span::default(self.props.notifications.len().to_string()).dim(),
            ]
            .to_vec(),
        );

        let progress = self
            .table
            .progress_percentage(self.props.notifications.len(), self.props.page_size);
        let progress = span::default(format!("{}%", progress)).dim();

        self.footer.render::<B>(
            frame,
            area,
            FooterProps {
                cells: [
                    filter.into(),
                    seen.clone().into(),
                    unseen.clone().into(),
                    sum.clone().into(),
                    progress.clone().into(),
                ],
                widths: [
                    Constraint::Fill(1),
                    Constraint::Min(seen.width() as u16),
                    Constraint::Min(unseen.width() as u16),
                    Constraint::Min(sum.width() as u16),
                    Constraint::Min(4),
                ],
                focus: self.props.focus,
                cutoff: self.props.cutoff,
                cutoff_after: self.props.cutoff_after,
            },
        );
    }
}

impl Render<()> for Notifications {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(area);

        self.render_header::<B>(frame, layout[0]);
        self.render_list::<B>(frame, layout[1]);
        self.render_footer::<B>(frame, layout[2]);

        let page_size = layout[1].height as usize;
        if page_size != self.props.page_size {
            let _ = self
                .action_tx
                .send(Action::PageSize(layout[1].height as usize));
        }
    }
}
