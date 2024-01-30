use radicle::issue::IssueId;
use tui::SelectionExit;
use tuirealm::command::{Cmd, CmdResult, Direction as MoveDirection};
use tuirealm::event::{Event, Key, KeyEvent};
use tuirealm::{MockComponent, NoUserEvent, State, StateValue};

use radicle_tui as tui;

use tui::ui::widget::container::{AppHeader, GlobalListener, LabeledContainer};
use tui::ui::widget::context::{ContextBar, Shortcuts};
use tui::ui::widget::list::PropertyList;

use tui::ui::widget::Widget;

use super::ui::{IdSelect, OperationSelect};
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

impl tuirealm::Component<Message, NoUserEvent> for Widget<IdSelect> {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        let mut submit = || -> Option<radicle::cob::patch::PatchId> {
            let result = self.perform(Cmd::Submit);
            match result {
                CmdResult::Submit(State::One(StateValue::Usize(selected))) => {
                    let item = self.items().get(selected)?;
                    Some(item.id().to_owned())
                }
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
                let output = SelectionExit::new(None, IssueId::from(id));
                Message::Quit(Some(output))
            }),
            _ => None,
        }
    }
}

impl tuirealm::Component<Message, NoUserEvent> for Widget<OperationSelect> {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        let mut submit = || -> Option<radicle::cob::patch::PatchId> {
            let result = self.perform(Cmd::Submit);
            match result {
                CmdResult::Submit(State::One(StateValue::Usize(selected))) => {
                    let item = self.items().get(selected)?;
                    Some(item.id().to_owned())
                }
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
                let exit = SelectionExit::new(
                    Some(format!("{}", IssueOperation::Show)),
                    IssueId::from(id),
                );
                Message::Quit(Some(exit))
            }),
            Event::Keyboard(KeyEvent {
                code: Key::Char('d'),
                ..
            }) => submit().map(|id| {
                let exit = SelectionExit::new(
                    Some(format!("{}", IssueOperation::Delete)),
                    IssueId::from(id),
                );
                Message::Quit(Some(exit))
            }),
            Event::Keyboard(KeyEvent {
                code: Key::Char('e'),
                ..
            }) => submit().map(|id| {
                let exit = SelectionExit::new(
                    Some(format!("{}", IssueOperation::Edit)),
                    IssueId::from(id),
                );
                Message::Quit(Some(exit))
            }),
            Event::Keyboard(KeyEvent {
                code: Key::Char('m'),
                ..
            }) => submit().map(|id| {
                let exit = SelectionExit::new(
                    Some(format!("{}", IssueOperation::Comment)),
                    IssueId::from(id),
                );
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
