use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Rect};

use radicle_tui as tui;

use tui::flux::ui::cob::NotificationItem;
use tui::flux::ui::widget::{
    Render, Shortcut, Shortcuts, ShortcutsProps, Table, TableProps, Widget,
};

use crate::tui_inbox::select::flux::{Action, InboxState};

pub struct ListPageProps {}

impl From<&InboxState> for ListPageProps {
    fn from(_state: &InboxState) -> Self {
        Self {}
    }
}

pub struct ListPage {
    /// Action sender
    pub action_tx: UnboundedSender<Action>,
    // Mapped Props from State
    _props: ListPageProps,
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
            _props: ListPageProps::from(state),
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
            _props: ListPageProps::from(state),
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
        let layout = tui::flux::ui::layout::default_page(area, 1u16, 1u16);

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
    table: Table<Action, NotificationItem, 7>,
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
        let header = [
            String::from(""),
            String::from(" ● "),
            String::from("Type"),
            String::from("Summary"),
            String::from("ID"),
            String::from("Status"),
            String::from("Updated"),
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

        self.table.render::<B>(
            frame,
            area,
            TableProps {
                items: self.props.notifications.to_vec(),
                header,
                widths: widths.to_vec(),
                focus: false,
            },
        );
    }
}
