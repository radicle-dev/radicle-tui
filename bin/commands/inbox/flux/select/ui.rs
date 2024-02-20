use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Rect};
use ratatui::widgets::Cell;

use radicle_tui as tui;

use tui::flux::ui::cob::NotificationItem;
use tui::flux::ui::span;
use tui::flux::ui::widget::{
    FooterProps, Render, Shortcut, Shortcuts, ShortcutsProps, Table, TableProps, Widget,
};

use super::{Action, InboxState};

pub struct ListPage {
    /// Action sender
    pub action_tx: UnboundedSender<Action>,
    /// notification widget
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
            ..self
        }
    }

    fn name(&self) -> &str {
        "list-page"
    }

    fn handle_key_event(&mut self, key: termion::event::Key) {
        match key {
            Key::Char('q') => {
                let _ = self.action_tx.send(Action::Exit);
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
    fn render<B: Backend>(&mut self, frame: &mut ratatui::Frame, _area: Rect, _props: ()) {
        let area = frame.size();
        let layout = tui::flux::ui::layout::default_page(area, 0u16, 1u16);

        self.notifications.render::<B>(frame, layout.component, ());
        self.shortcuts.render::<B>(
            frame,
            layout.shortcuts,
            ShortcutsProps {
                shortcuts: vec![Shortcut::new("enter", "select"), Shortcut::new("q", "quit")],
                divider: '∙',
            },
        );
    }
}

struct NotificationsProps {
    notifications: Vec<NotificationItem>,
}

impl From<&InboxState> for NotificationsProps {
    fn from(state: &InboxState) -> Self {
        Self {
            notifications: state.notifications.clone(),
        }
    }
}

struct Notifications {
    /// Sending actions to the state store
    action_tx: UnboundedSender<Action>,
    /// State Mapped RoomList Props
    props: NotificationsProps,
    /// Notification table
    table: Table<Action>,
}

impl Widget<InboxState, Action> for Notifications {
    fn new(state: &InboxState, action_tx: UnboundedSender<Action>) -> Self {
        Self {
            action_tx: action_tx.clone(),
            props: NotificationsProps::from(state),
            table: Table::new(state, action_tx.clone()),
        }
    }

    fn move_with_state(self, state: &InboxState) -> Self
    where
        Self: Sized,
    {
        Self {
            props: NotificationsProps::from(state),
            table: self.table.move_with_state(state),
            ..self
        }
    }

    fn name(&self) -> &str {
        "notification-list"
    }

    fn handle_key_event(&mut self, key: Key) {
        match key {
            Key::Up => {
                self.table.prev();

                let selected = self
                    .table
                    .selected()
                    .and_then(|selected| self.props.notifications.get(selected));

                // TODO: propagate error
                if let Some(notif) = selected {
                    let _ = self.action_tx.send(Action::Select(notif.id));
                }
            }
            Key::Down => {
                self.table.next(self.props.notifications.len());

                let selected = self
                    .table
                    .selected()
                    .and_then(|selected| self.props.notifications.get(selected));

                // TODO: propagate error
                if let Some(notif) = selected {
                    let _ = self.action_tx.send(Action::Select(notif.id));
                }
            }
            _ => {}
        }
    }
}

impl Render<()> for Notifications {
    fn render<B: Backend>(&mut self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let header: [Cell; 7] = [
            String::from("").into(),
            String::from(" ● ").into(),
            String::from("Type").into(),
            String::from("Summary").into(),
            String::from("ID").into(),
            String::from("Status").into(),
            String::from("Updated").into(),
        ];

        let widths = [
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Length(6),
            Constraint::Fill(1),
            Constraint::Length(15),
            Constraint::Length(10),
            Constraint::Length(18),
        ];

        let progress = {
            let step = self
                .table
                .selected()
                .map(|selected| selected.saturating_add(1).to_string())
                .unwrap_or("-".to_string());
            let length = self.props.notifications.len().to_string();

            span::badge(format!("{}/{}", step, length))
        };

        let footer = FooterProps {
            cells: [
                span::badge("/".to_string()),
                String::from("").into(),
                String::from("").into(),
                progress.clone(),
            ]
            .to_vec(),
            widths: [
                Constraint::Length(3),
                Constraint::Fill(1),
                Constraint::Fill(1),
                Constraint::Length(progress.width() as u16),
            ]
            .to_vec(),
        };

        self.table.render::<B>(
            frame,
            area,
            TableProps {
                items: self.props.notifications.to_vec(),
                focus: false,
                widths,
                header,
                footer: Some(footer),
            },
        );
    }
}
