use std::cmp;
use std::vec;

use ratatui::style::Stylize;
use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

use radicle_tui as tui;

use tui::common::cob::patch::Filter;
use tui::flux::ui::cob::PatchItem;
use tui::flux::ui::span;
use tui::flux::ui::widget::container::{Footer, FooterProps, Header, HeaderProps};
use tui::flux::ui::widget::{
    Render, Shortcut, Shortcuts, ShortcutsProps, Table, TableProps, Widget,
};
use tui::Selection;

use crate::tui_patch::common::Mode;
use crate::tui_patch::common::PatchOperation;

use super::{Action, PatchesState};

pub struct ListPageProps {
    selected: Option<PatchItem>,
    mode: Mode,
}

impl From<&PatchesState> for ListPageProps {
    fn from(state: &PatchesState) -> Self {
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
    patches: Patches,
    /// Shortcut widget
    shortcuts: Shortcuts<Action>,
}

impl Widget<PatchesState, Action> for ListPage {
    fn new(state: &PatchesState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
            props: ListPageProps::from(state),
            patches: Patches::new(state, action_tx.clone()),
            shortcuts: Shortcuts::new(state, action_tx.clone()),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &PatchesState) -> Self
    where
        Self: Sized,
    {
        ListPage {
            patches: self.patches.move_with_state(state),
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
            Key::Char('q') | Key::Ctrl('c') => {
                let _ = self.action_tx.send(Action::Exit { selection: None });
            }
            Key::Char('\n') => {
                if let Some(selected) = &self.props.selected {
                    let operation = match self.props.mode {
                        Mode::Operation => Some(PatchOperation::Show.to_string()),
                        Mode::Id => None,
                    };
                    let _ = self.action_tx.send(Action::Exit {
                        selection: Some(Selection {
                            operation,
                            ids: vec![selected.id],
                            args: vec![],
                        }),
                    });
                }
            }
            Key::Char('c') => {
                if let Some(selected) = &self.props.selected {
                    let _ = self.action_tx.send(Action::Exit {
                        selection: Some(Selection {
                            operation: Some(PatchOperation::Checkout.to_string()),
                            ids: vec![selected.id],
                            args: vec![],
                        }),
                    });
                }
            }
            Key::Char('m') => {
                if let Some(selected) = &self.props.selected {
                    let _ = self.action_tx.send(Action::Exit {
                        selection: Some(Selection {
                            operation: Some(PatchOperation::Comment.to_string()),
                            ids: vec![selected.id],
                            args: vec![],
                        }),
                    });
                }
            }
            Key::Char('e') => {
                if let Some(selected) = &self.props.selected {
                    let _ = self.action_tx.send(Action::Exit {
                        selection: Some(Selection {
                            operation: Some(PatchOperation::Edit.to_string()),
                            ids: vec![selected.id],
                            args: vec![],
                        }),
                    });
                }
            }
            Key::Char('d') => {
                if let Some(selected) = &self.props.selected {
                    let _ = self.action_tx.send(Action::Exit {
                        selection: Some(Selection {
                            operation: Some(PatchOperation::Delete.to_string()),
                            ids: vec![selected.id],
                            args: vec![],
                        }),
                    });
                }
            }
            _ => {
                <Patches as Widget<PatchesState, Action>>::handle_key_event(&mut self.patches, key);
            }
        }
    }
}

impl Render<()> for ListPage {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, _area: Rect, _props: ()) {
        let area = frame.size();
        let layout = tui::flux::ui::layout::default_page(area, 0u16, 1u16);

        let shortcuts = match self.props.mode {
            Mode::Id => vec![Shortcut::new("enter", "select"), Shortcut::new("q", "quit")],
            Mode::Operation => vec![
                Shortcut::new("enter", "show"),
                Shortcut::new("c", "checkout"),
                Shortcut::new("m", "comment"),
                Shortcut::new("e", "edit"),
                Shortcut::new("d", "delete"),
                Shortcut::new("q", "quit"),
            ],
        };

        self.patches.render::<B>(frame, layout.component, ());
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

struct PatchesProps {
    patches: Vec<PatchItem>,
    filter: Filter,
}

impl From<&PatchesState> for PatchesProps {
    fn from(state: &PatchesState) -> Self {
        Self {
            patches: state.patches.clone(),
            filter: state.filter.clone(),
        }
    }
}

struct Patches {
    /// Action sender
    action_tx: UnboundedSender<Action>,
    /// State mapped props
    props: PatchesProps,
    /// Table header
    header: Header<Action>,
    /// Notification table
    table: Table<Action>,
    /// Table footer
    footer: Footer<Action>,
}

impl Widget<PatchesState, Action> for Patches {
    fn new(state: &PatchesState, action_tx: UnboundedSender<Action>) -> Self {
        Self {
            action_tx: action_tx.clone(),
            props: PatchesProps::from(state),
            header: Header::new(state, action_tx.clone()),
            table: Table::new(state, action_tx.clone()),
            footer: Footer::new(state, action_tx),
        }
    }

    fn move_with_state(self, state: &PatchesState) -> Self
    where
        Self: Sized,
    {
        Self {
            props: PatchesProps::from(state),
            header: self.header.move_with_state(state),
            table: self.table.move_with_state(state),
            footer: self.footer.move_with_state(state),
            ..self
        }
    }

    fn name(&self) -> &str {
        "patches"
    }

    fn handle_key_event(&mut self, key: Key) {
        match key {
            Key::Up => {
                self.table.prev();

                let selected = self
                    .table
                    .selected()
                    .and_then(|selected| self.props.patches.get(selected));

                // TODO: propagate error
                if let Some(notif) = selected {
                    let _ = self.action_tx.send(Action::Select {
                        item: notif.clone(),
                    });
                }
            }
            Key::Down => {
                self.table.next(self.props.patches.len());

                let selected = self
                    .table
                    .selected()
                    .and_then(|selected| self.props.patches.get(selected));

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

impl Render<()> for Patches {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let cutoff = 200;
        let cutoff_after = 5;
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
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Fill(1),
            Constraint::Length(16),
            Constraint::Length(16),
            Constraint::Length(8),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(16),
        ];

        let progress = {
            let step = self
                .table
                .selected()
                .map(|selected| selected.saturating_add(1).to_string())
                .unwrap_or("-".to_string());
            let length = self.props.patches.len().to_string();

            span::badge(format!("{}/{}", cmp::min(&step, &length), length))
        };

        self.header.render::<B>(
            frame,
            layout[0],
            HeaderProps {
                cells: [
                    String::from(" ● ").into(),
                    String::from("ID").into(),
                    String::from("Title").into(),
                    String::from("Author").into(),
                    String::from("").into(),
                    String::from("Head").into(),
                    String::from("+").into(),
                    String::from("- ").into(),
                    String::from("Updated").into(),
                ],
                widths,
                focus,
                cutoff,
                cutoff_after,
            },
        );

        self.table.render::<B>(
            frame,
            layout[1],
            TableProps {
                items: self.props.patches.to_vec(),
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
                cells: [
                    span::badge("/".to_string()),
                    span::default(self.props.filter.to_string()).magenta().dim(),
                    String::from("").into(),
                    progress.clone(),
                ],
                widths: [
                    Constraint::Length(3),
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
