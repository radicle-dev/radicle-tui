use radicle::issue::IssueId;

use tuirealm::command::{Cmd, CmdResult, Direction as MoveDirection};
use tuirealm::event::{Event, Key, KeyEvent};
use tuirealm::{MockComponent, NoUserEvent};

use radicle_tui as tui;

use tui::ui::widget::container::{AppHeader, GlobalListener, LabeledContainer};
use tui::ui::widget::context::{ContextBar, Shortcuts};
use tui::ui::widget::list::PropertyList;
use tui::ui::widget::Widget;
use tui::SelectionExit;

use super::ui::OperationSelect;
use super::{IssueOperation, Message};

/// Since the framework does not know the type of messages that are being
/// passed around in the app, the following handlers need to be implemented for
/// each component used.
///
/// TODO: should handle `Event::WindowResize`, which is not emitted by `termion`.
impl tuirealm::Component<Message, NoUserEvent> for Widget<GlobalListener> {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        match event {
            Event::Keyboard(KeyEvent {
                code: Key::Char('q'),
                ..
            }) => Some(Message::Quit(None)),
            _ => None,
        }
    }
}

impl tuirealm::Component<Message, NoUserEvent> for Widget<OperationSelect> {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        let mut submit = || -> Option<radicle::cob::patch::PatchId> {
            match self.perform(Cmd::Submit) {
                CmdResult::Submit(state) => None,
                _ => None,
            }
        };

        match event {
            Event::Keyboard(KeyEvent { code: Key::Up, .. })
            | Event::Keyboard(KeyEvent {
                code: Key::Char('k'),
                ..
            }) => {
                self.perform(Cmd::Move(MoveDirection::Up));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Down, ..
            })
            | Event::Keyboard(KeyEvent {
                code: Key::Char('j'),
                ..
            }) => {
                self.perform(Cmd::Move(MoveDirection::Down));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Enter, ..
            }) => submit().map(|id| {
                let exit = SelectionExit::default()
                    .with_operation(IssueOperation::Show.to_string())
                    .with_id(IssueId::from(id));
                Message::Quit(Some(exit))
            }),
            Event::Keyboard(KeyEvent {
                code: Key::Char('d'),
                ..
            }) => submit().map(|id| {
                let exit = SelectionExit::default()
                    .with_operation(IssueOperation::Delete.to_string())
                    .with_id(IssueId::from(id));
                Message::Quit(Some(exit))
            }),
            Event::Keyboard(KeyEvent {
                code: Key::Char('e'),
                ..
            }) => submit().map(|id| {
                let exit = SelectionExit::default()
                    .with_operation(IssueOperation::Edit.to_string())
                    .with_id(IssueId::from(id));
                Message::Quit(Some(exit))
            }),
            Event::Keyboard(KeyEvent {
                code: Key::Char('m'),
                ..
            }) => submit().map(|id| {
                let exit = SelectionExit::default()
                    .with_operation(IssueOperation::Comment.to_string())
                    .with_id(IssueId::from(id));
                Message::Quit(Some(exit))
            }),
            _ => None,
        }
    }
}

impl tuirealm::Component<Message, NoUserEvent> for Widget<AppHeader> {
    fn on(&mut self, _event: Event<NoUserEvent>) -> Option<Message> {
        None
    }
}

impl tuirealm::Component<Message, NoUserEvent> for Widget<LabeledContainer> {
    fn on(&mut self, _event: Event<NoUserEvent>) -> Option<Message> {
        None
    }
}

impl tuirealm::Component<Message, NoUserEvent> for Widget<PropertyList> {
    fn on(&mut self, _event: Event<NoUserEvent>) -> Option<Message> {
        None
    }
}

impl tuirealm::Component<Message, NoUserEvent> for Widget<ContextBar> {
    fn on(&mut self, _event: Event<NoUserEvent>) -> Option<Message> {
        None
    }
}

impl tuirealm::Component<Message, NoUserEvent> for Widget<Shortcuts> {
    fn on(&mut self, _event: Event<NoUserEvent>) -> Option<Message> {
        None
    }
}
