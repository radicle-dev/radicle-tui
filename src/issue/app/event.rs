use radicle::cob::issue::IssueId;

use tuirealm::command::{Cmd, CmdResult, Direction as MoveDirection};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
use tuirealm::{MockComponent, NoUserEvent, State, StateValue};

use radicle_tui as tui;

use tui::ui::widget::container::{AppHeader, GlobalListener, LabeledContainer, Popup};
use tui::ui::widget::context::{ContextBar, Shortcuts};
use tui::ui::widget::list::PropertyList;

use tui::ui::widget::Widget;

use super::ui::{self, EditForm, OpenForm};
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
            Event::Keyboard(KeyEvent { code: Key::Esc, .. }) => match self.state() {
                State::Tup2((StateValue::Usize(selected), StateValue::Usize(_))) => {
                    let item = self.items().get(selected)?;
                    Some(Message::Issue(IssueMessage::Leave(Some(
                        item.id().to_owned(),
                    ))))
                }
                _ => None,
            },
            Event::Keyboard(KeyEvent { code: Key::Up, .. })
            | Event::Keyboard(KeyEvent {
                code: Key::Char('k'),
                ..
            }) => {
                let result = self.perform(Cmd::Move(MoveDirection::Up));
                match result {
                    CmdResult::Changed(State::One(StateValue::Usize(selected))) => {
                        let item = self.items().get(selected)?;
                        Some(Message::Issue(IssueMessage::Changed(item.id().to_owned())))
                    }
                    _ => None,
                }
            }
            Event::Keyboard(KeyEvent {
                code: Key::Down, ..
            })
            | Event::Keyboard(KeyEvent {
                code: Key::Char('j'),
                ..
            }) => {
                let result = self.perform(Cmd::Move(MoveDirection::Down));
                match result {
                    CmdResult::Changed(State::One(StateValue::Usize(selected))) => {
                        let item = self.items().get(selected)?;
                        Some(Message::Issue(IssueMessage::Changed(item.id().to_owned())))
                    }
                    _ => None,
                }
            }
            Event::Keyboard(KeyEvent {
                code: Key::Enter, ..
            }) => Some(Message::Issue(IssueMessage::Focus(IssueCid::Details))),
            Event::Keyboard(KeyEvent {
                code: Key::Char('o'),
                ..
            }) => Some(Message::Issue(IssueMessage::ShowOpenForm)),
            Event::Keyboard(KeyEvent {
                code: Key::Char('e'),
                ..
            }) => match self.state() {
                State::Tup2((StateValue::Usize(selected), StateValue::Usize(_))) => {
                    let item = self.items().get(selected)?;
                    Some(Message::Issue(IssueMessage::ShowEditForm(
                        item.id().to_owned(),
                    )))
                }
                _ => None,
            },
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

impl tuirealm::Component<Message, NoUserEvent> for Widget<OpenForm> {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        match event {
            Event::Keyboard(KeyEvent {
                code: Key::Char('s'),
                modifiers: KeyModifiers::CONTROL,
            }) => {
                self.perform(Cmd::Submit);
                match self.state() {
                    State::Linked(mut fields) => {
                        let mut missing_values = vec![];

                        let title = match fields.pop_front() {
                            Some(State::One(StateValue::String(title))) if !title.is_empty() => {
                                Some(title)
                            }
                            _ => None,
                        };

                        let labels = match fields.pop_front() {
                            Some(State::One(StateValue::String(labels))) => Some(labels),
                            _ => Some(String::from("[]")),
                        };

                        let assignees = match fields.pop_front() {
                            Some(State::One(StateValue::String(assignees))) => Some(assignees),
                            _ => Some(String::from("[]")),
                        };

                        let description = match fields.pop_front() {
                            Some(State::One(StateValue::String(description)))
                                if !description.is_empty() =>
                            {
                                Some(description)
                            }
                            _ => None,
                        };

                        if title.is_none() {
                            missing_values.push("title");
                        }
                        if description.is_none() {
                            missing_values.push("description");
                        }

                        // show error popup if missing.
                        if missing_values.is_empty() {
                            Some(Message::Issue(IssueMessage::Cob(
                                super::IssueCobMessage::Create {
                                    title: title.unwrap(),
                                    labels: labels.unwrap(),
                                    assignees: assignees.unwrap(),
                                    description: description.unwrap(),
                                },
                            )))
                        } else {
                            let error = format!("Missing fields: {:?}", missing_values);
                            Some(Message::Popup(PopupMessage::Error(error)))
                        }
                    }
                    _ => None,
                }
            }
            _ => form::perform(event, self),
        }
    }
}

impl tuirealm::Component<Message, NoUserEvent> for Widget<EditForm> {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        match event {
            Event::Keyboard(KeyEvent {
                code: Key::Char('s'),
                modifiers: KeyModifiers::CONTROL,
            }) => {
                self.perform(Cmd::Submit);
                match self.state() {
                    State::Linked(mut fields) => {
                        let mut missing_values = vec![];

                        let id = match fields.pop_front() {
                            Some(State::One(StateValue::String(id))) => Some(id),
                            _ => None,
                        };

                        let title = match fields.pop_front() {
                            Some(State::One(StateValue::String(title))) if !title.is_empty() => {
                                Some(title)
                            }
                            _ => None,
                        };

                        let labels = match fields.pop_front() {
                            Some(State::One(StateValue::String(labels))) => Some(labels),
                            _ => Some(String::from("[]")),
                        };

                        let assignees = match fields.pop_front() {
                            Some(State::One(StateValue::String(assignees))) => Some(assignees),
                            _ => Some(String::from("[]")),
                        };

                        let description = match fields.pop_front() {
                            Some(State::One(StateValue::String(description)))
                                if !description.is_empty() =>
                            {
                                Some(description)
                            }
                            _ => None,
                        };

                        let state = match fields.pop_front() {
                            Some(State::One(StateValue::Usize(index))) => Some(index),
                            _ => None,
                        };

                        if title.is_none() {
                            missing_values.push("title");
                        }
                        if description.is_none() {
                            missing_values.push("description");
                        }

                        // show error popup if missing.
                        if missing_values.is_empty() {
                            Some(Message::Issue(IssueMessage::Cob(
                                super::IssueCobMessage::Edit {
                                    id: id.unwrap(),
                                    title: title.unwrap(),
                                    labels: labels.unwrap(),
                                    assignees: assignees.unwrap(),
                                    description: description.unwrap(),
                                    state: state.unwrap() as u16,
                                },
                            )))
                        } else {
                            let error = format!("Missing fields: {:?}", missing_values);
                            Some(Message::Popup(PopupMessage::Error(error)))
                        }
                    }
                    _ => None,
                }
            }
            _ => form::perform(event, self),
        }
    }
}

impl tuirealm::Component<Message, NoUserEvent> for Widget<ui::IssueBrowser> {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        let mut submit = || -> Option<IssueId> {
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
                code: Key::Char('o'),
                ..
            }) => {
                let id = submit();
                Some(Message::Batch(vec![
                    Message::Issue(IssueMessage::Show(id)),
                    Message::Issue(IssueMessage::ShowOpenForm),
                ]))
            }
            Event::Keyboard(KeyEvent {
                code: Key::Char('e'),
                ..
            }) => {
                let id = submit();
                Some(Message::Batch(vec![
                    Message::Issue(IssueMessage::Show(id)),
                    Message::Issue(IssueMessage::ShowEditForm(id.unwrap())),
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

mod form {
    use tuirealm::command::{Cmd, Direction as MoveDirection, Position};
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
    use tuirealm::{MockComponent, NoUserEvent};

    use radicle_tui as tui;

    use tui::ui::widget::form::Form;
    use tui::ui::widget::{Widget, WidgetComponent};

    use super::{IssueMessage, Message};

    pub fn perform<T: WidgetComponent>(
        event: Event<NoUserEvent>,
        widget: &mut Widget<T>,
    ) -> Option<Message> {
        match event {
            Event::Keyboard(KeyEvent {
                code: Key::Left, ..
            }) => {
                widget.perform(Cmd::Move(MoveDirection::Left));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Right, ..
            }) => {
                widget.perform(Cmd::Move(MoveDirection::Right));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent { code: Key::Up, .. }) => {
                widget.perform(Cmd::Move(MoveDirection::Up));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Down, ..
            }) => {
                widget.perform(Cmd::Move(MoveDirection::Down));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Home, ..
            }) => {
                widget.perform(Cmd::GoTo(Position::Begin));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent { code: Key::End, .. }) => {
                widget.perform(Cmd::GoTo(Position::End));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Delete, ..
            }) => {
                widget.perform(Cmd::Cancel);
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Backspace,
                ..
            }) => {
                widget.perform(Cmd::Delete);
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Enter, ..
            }) => {
                widget.perform(Cmd::Custom(Form::CMD_ENTER));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::BackTab, ..
            }) => {
                widget.perform(Cmd::Custom(Form::CMD_FOCUS_PREVIOUS));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent { code: Key::Tab, .. }) => {
                widget.perform(Cmd::Custom(Form::CMD_FOCUS_NEXT));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Char('v'),
                modifiers: KeyModifiers::CONTROL,
            }) => {
                widget.perform(Cmd::Custom(Form::CMD_PASTE));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Char(ch),
                modifiers: KeyModifiers::SHIFT,
            }) => {
                widget.perform(Cmd::Type(ch.to_ascii_uppercase()));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Char(ch),
                ..
            }) => {
                widget.perform(Cmd::Type(ch));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent { code: Key::Esc, .. }) => {
                Some(Message::Issue(IssueMessage::HideForm))
            }
            _ => None,
        }
    }
}
