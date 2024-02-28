use std::collections::HashMap;
use std::vec;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::backend::Backend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
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

use super::common::Mode;
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
                    let selection = match self.props.mode {
                        Mode::Operation => Selection::default()
                            .with_operation("show".to_string())
                            .with_id(selected.id),
                        Mode::Id => Selection::default().with_id(selected.id),
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

        let shortcuts = match self.props.mode {
            Mode::Id => vec![Shortcut::new("enter", "select")],
            Mode::Operation => vec![
                Shortcut::new("enter", "show"),
                Shortcut::new("c", "clear"),
            ],
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
    project: Project,
    stats: HashMap<String, usize>,
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
            project: state.project.clone(),
            stats,
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

impl Render<()> for Notifications {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let cutoff = 200;
        let cutoff_after = 8;
        let focus = false;

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(area);

        let widths = [
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Length(20),
            Constraint::Fill(1),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(15),
            Constraint::Length(18),
        ];

        let filter = Line::from([span::blank()].to_vec());
        let stats = Line::from(
            [
                span::positive(self.props.stats.get("Seen").unwrap_or(&0).to_string()).dim(),
                span::default(" Seen".to_string()).dim(),
                span::default(" | ".to_string()).dim(),
                span::default(self.props.stats.get("Unseen").unwrap_or(&0).to_string())
                    .magenta()
                    .dim(),
                span::default(" Unseen".to_string()).dim(),
            ]
            .to_vec(),
        )
        .alignment(Alignment::Right);

        let (step, len) = self.table.progress(self.props.notifications.len());
        let progress = span::progress(step, len, false);

        self.header.render::<B>(
            frame,
            layout[0],
            HeaderProps {
                cells: [String::from("").into(), self.props.project.name().into()],
                widths: [Constraint::Length(0), Constraint::Fill(1)],
                focus,
                cutoff,
                cutoff_after,
            },
        );

        self.table.render::<B>(
            frame,
            layout[1],
            TableProps {
                items: self.props.notifications.to_vec(),
                has_header: true,
                has_footer: true,
                focus,
                widths,
                cutoff,
                cutoff_after,
            },
        );

        self.footer.render::<B>(
            frame,
            layout[2],
            FooterProps {
                cells: [filter.into(), stats.into(), progress.clone().into()],
                widths: [
                    Constraint::Fill(1),
                    Constraint::Fill(1),
                    Constraint::Length(progress.width() as u16),
                ],
                focus,
                cutoff,
                cutoff_after,
            },
        );
    }
}
