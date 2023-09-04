use radicle::cob::issue::IssueId;
use tuirealm::command::{Cmd, CmdResult, Direction as MoveDirection, Position};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
use tuirealm::{MockComponent, NoUserEvent, State, StateValue};

use radicle_tui::ui::widget::common::container::{
    AppHeader, GlobalListener, LabeledContainer, Popup,
};
use radicle_tui::ui::widget::common::context::{ContextBar, Shortcuts};
use radicle_tui::ui::widget::common::form::{Form, TextInput};
use radicle_tui::ui::widget::common::list::PropertyList;
use radicle_tui::ui::widget::home::{Dashboard, IssueBrowser, PatchBrowser};
use radicle_tui::ui::widget::issue::NewForm;
use radicle_tui::ui::widget::{issue, patch};

use radicle_tui::ui::widget::Widget;

use super::{IssueCid, IssueCobMessage, IssueMessage, Message, PatchMessage, PopupMessage};

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

impl tuirealm::Component<Message, NoUserEvent> for Widget<issue::LargeList> {
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
            }) => Some(Message::Issue(IssueMessage::OpenForm)),
            _ => None,
        }
    }
}

impl tuirealm::Component<Message, NoUserEvent> for Widget<issue::IssueDetails> {
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

impl tuirealm::Component<Message, NoUserEvent> for Widget<issue::NewForm> {
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
                self.perform(Cmd::Custom(TextInput::CMD_NEWLINE));
                Some(Message::Tick)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Char('s'),
                modifiers: KeyModifiers::CONTROL,
            }) => {
                match self.perform(Cmd::Submit) {
                    CmdResult::Submit(State::Map(inputs)) => {
                        let mut missing_values = vec![];

                        let title = match inputs.get(NewForm::INPUT_TITLE) {
                            Some(StateValue::String(title)) if !title.is_empty() => {
                                Some(title.clone())
                            }
                            _ => None,
                        };
                        let tags = match inputs.get(NewForm::INPUT_TAGS) {
                            Some(StateValue::String(tags)) => Some(tags.clone()),
                            _ => Some(String::from("[]")),
                        };
                        let assignees = match inputs.get(NewForm::INPUT_ASSIGNESS) {
                            Some(StateValue::String(assignees)) => Some(assignees.clone()),
                            _ => Some(String::from("[]")),
                        };
                        let description = match inputs.get(NewForm::INPUT_DESCRIPTION) {
                            Some(StateValue::String(description)) if !description.is_empty() => {
                                Some(description.clone())
                            }
                            _ => None,
                        };

                        if title.is_none() {
                            missing_values.push(NewForm::INPUT_TITLE);
                        }
                        if description.is_none() {
                            missing_values.push(NewForm::INPUT_DESCRIPTION);
                        }

                        // show error popup if missing.
                        if !missing_values.is_empty() {
                            let error = format!("Missing fields: {:?}", missing_values);
                            Some(Message::Popup(PopupMessage::Error(error)))
                        } else {
                            Some(Message::Issue(IssueMessage::Cob(IssueCobMessage::Create {
                                title: title.unwrap(),
                                tags: tags.unwrap(),
                                assignees: assignees.unwrap(),
                                description: description.unwrap(),
                            })))
                        }
                    }
                    _ => None,
                }
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
                self.perform(Cmd::Custom(TextInput::CMD_PASTE));
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

impl tuirealm::Component<Message, NoUserEvent> for Widget<PatchBrowser> {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
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
            }) => {
                let result = self.perform(Cmd::Submit);
                match result {
                    CmdResult::Submit(State::One(StateValue::Usize(selected))) => {
                        let item = self.items().get(selected)?;
                        Some(Message::Patch(PatchMessage::Show(item.id().to_owned())))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

impl tuirealm::Component<Message, NoUserEvent> for Widget<IssueBrowser> {
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

impl tuirealm::Component<Message, NoUserEvent> for Widget<Dashboard> {
    fn on(&mut self, _event: Event<NoUserEvent>) -> Option<Message> {
        None
    }
}

impl tuirealm::Component<Message, NoUserEvent> for Widget<patch::Activity> {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        match event {
            Event::Keyboard(KeyEvent { code: Key::Esc, .. }) => {
                Some(Message::Patch(PatchMessage::Leave))
            }
            _ => None,
        }
    }
}

impl tuirealm::Component<Message, NoUserEvent> for Widget<patch::Files> {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        match event {
            Event::Keyboard(KeyEvent { code: Key::Esc, .. }) => {
                Some(Message::Patch(PatchMessage::Leave))
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
