use radicle::cob::issue::IssueId;

use tuirealm::command::{Cmd, CmdResult, Direction as MoveDirection, Position};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
use tuirealm::{MockComponent, NoUserEvent, State, StateValue};

use radicle_tui as tui;

use tui::realm::ui::state::ItemState;
use tui::realm::ui::widget::container::{AppHeader, GlobalListener, LabeledContainer, Popup};
use tui::realm::ui::widget::context::{ContextBar, Shortcuts};
use tui::realm::ui::widget::form::Form;
use tui::realm::ui::widget::list::PropertyList;
use tui::realm::ui::widget::Widget;

use super::ui;
use super::{IssueCid, IssueMessage, Message, PopupMessage};

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
            }) => Some(Message::Quit),
            _ => None,
        }
    }
}

impl tuirealm::Component<Message, NoUserEvent> for Widget<AppHeader> {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        match event {
            Event::Keyboard(KeyEvent { code: Key::Tab, .. }) => {
                match self.perform(Cmd::Move(MoveDirection::Right)) {
                    CmdResult::Changed(State::One(StateValue::U16(index))) => {
                        Some(Message::NavigationChanged(index))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

impl tuirealm::Component<Message, NoUserEvent> for Widget<ui::LargeList> {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        match event {
            Event::Keyboard(KeyEvent { code: Key::Esc, .. }) => {
                let selected = ItemState::try_from(self.state()).ok()?.selected()?;
                let item = self.items().get(selected)?;

                Some(Message::Issue(IssueMessage::Leave(Some(
                    item.id().to_owned(),
                ))))
            }
            Event::Keyboard(KeyEvent { code: Key::Up, .. })
            | Event::Keyboard(KeyEvent {
                code: Key::Char('k'),
                ..
            }) => match self.perform(Cmd::Move(MoveDirection::Up)) {
                CmdResult::Changed(state) => {
                    let selected = ItemState::try_from(state).ok()?.selected()?;
                    let item = self.items().get(selected)?;

                    Some(Message::Issue(IssueMessage::Changed(item.id().to_owned())))
                }
                _ => None,
            },
            Event::Keyboard(KeyEvent {
                code: Key::Down, ..
            })
            | Event::Keyboard(KeyEvent {
                code: Key::Char('j'),
                ..
            }) => match self.perform(Cmd::Move(MoveDirection::Down)) {
                CmdResult::Changed(state) => {
                    let selected = ItemState::try_from(state).ok()?.selected()?;
                    let item = self.items().get(selected)?;
                    Some(Message::Issue(IssueMessage::Changed(item.id().to_owned())))
                }
                _ => None,
            },
            Event::Keyboard(KeyEvent {
                code: Key::Enter, ..
            }) => Some(Message::Issue(IssueMessage::Focus(IssueCid::Details))),
            Event::Keyboard(KeyEvent {
                code: Key::Char('o'),
                ..
            }) => Some(Message::Issue(IssueMessage::OpenForm)),
            _ => None,
        }
    }
}

impl tuirealm::Component<Message, NoUserEvent> for Widget<ui::IssueDetails> {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        match event {
            Event::Keyboard(KeyEvent { code: Key::Up, .. })
            | Event::Keyboard(KeyEvent {
                code: Key::Char('k'),
                ..
            }) => {
                self.perform(Cmd::Scroll(MoveDirection::Up));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Down, ..
            })
            | Event::Keyboard(KeyEvent {
                code: Key::Char('j'),
                ..
            }) => {
                self.perform(Cmd::Scroll(MoveDirection::Down));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent { code: Key::Esc, .. }) => {
                Some(Message::Issue(IssueMessage::Focus(IssueCid::List)))
            }
            _ => None,
        }
    }
}

impl tuirealm::Component<Message, NoUserEvent> for Widget<Form> {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        match event {
            Event::Keyboard(KeyEvent {
                code: Key::Left, ..
            }) => {
                self.perform(Cmd::Move(MoveDirection::Left));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Right, ..
            }) => {
                self.perform(Cmd::Move(MoveDirection::Right));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent { code: Key::Up, .. }) => {
                self.perform(Cmd::Move(MoveDirection::Up));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Down, ..
            }) => {
                self.perform(Cmd::Move(MoveDirection::Down));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Home, ..
            }) => {
                self.perform(Cmd::GoTo(Position::Begin));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent { code: Key::End, .. }) => {
                self.perform(Cmd::GoTo(Position::End));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Delete, ..
            }) => {
                self.perform(Cmd::Cancel);
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Backspace,
                ..
            }) => {
                self.perform(Cmd::Delete);
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Enter, ..
            }) => {
                self.perform(Cmd::Custom(Form::CMD_NEWLINE));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Char('s'),
                modifiers: KeyModifiers::CONTROL,
            }) => {
                self.perform(Cmd::Submit);
                self.query(tuirealm::Attribute::Custom(Form::PROP_ID))
                    .map(|cid| Message::FormSubmitted(cid.unwrap_string()))
            }
            Event::Keyboard(KeyEvent { code: Key::Esc, .. }) => {
                Some(Message::Issue(IssueMessage::HideForm))
            }
            Event::Keyboard(KeyEvent {
                code: Key::BackTab, ..
            }) => {
                self.perform(Cmd::Custom(Form::CMD_FOCUS_PREVIOUS));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent { code: Key::Tab, .. }) => {
                self.perform(Cmd::Custom(Form::CMD_FOCUS_NEXT));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Char(ch),
                modifiers: KeyModifiers::SHIFT,
            }) => {
                self.perform(Cmd::Type(ch.to_ascii_uppercase()));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Char('v'),
                modifiers: KeyModifiers::CONTROL,
            }) => {
                self.perform(Cmd::Custom(Form::CMD_PASTE));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Char(ch),
                ..
            }) => {
                self.perform(Cmd::Type(ch));
                Some(Message::Tick)
            }
            _ => None,
        }
    }
}

impl tuirealm::Component<Message, NoUserEvent> for Widget<ui::IssueBrowser> {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        let mut submit = || -> Option<IssueId> {
            match self.perform(Cmd::Submit) {
                CmdResult::Submit(state) => {
                    let selected = ItemState::try_from(state).ok()?.selected()?;
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
                code: Key::Char('o'),
                ..
            }) => {
                let id = submit();
                Some(Message::Batch(vec![
                    Message::Issue(IssueMessage::Show(id)),
                    Message::Issue(IssueMessage::OpenForm),
                ]))
            }
            Event::Keyboard(KeyEvent {
                code: Key::Enter, ..
            }) => {
                let id = submit();
                if id.is_some() {
                    Some(Message::Issue(IssueMessage::Show(id)))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

impl tuirealm::Component<Message, NoUserEvent> for Widget<Popup> {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        match event {
            Event::Keyboard(KeyEvent { code: Key::Esc, .. }) => {
                Some(Message::Popup(PopupMessage::Hide))
            }
            _ => None,
        }
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
