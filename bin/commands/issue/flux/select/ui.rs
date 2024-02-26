use std::collections::HashMap;
use std::vec;

use radicle::issue;
use ratatui::style::Stylize;
use ratatui::text::Line;
use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::backend::Backend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};

use radicle_tui as tui;

use tui::common::cob::issue::Filter;
use tui::flux::ui::cob::IssueItem;
use tui::flux::ui::span;
use tui::flux::ui::widget::container::{Footer, FooterProps, Header, HeaderProps};
use tui::flux::ui::widget::{
    Render, Shortcut, Shortcuts, ShortcutsProps, Table, TableProps, Widget,
};
use tui::Selection;

use crate::tui_issue::common::IssueOperation;
use crate::tui_issue::common::Mode;

use super::{Action, IssuesState};

pub struct ListPageProps {
    selected: Option<IssueItem>,
    mode: Mode,
}

impl From<&IssuesState> for ListPageProps {
    fn from(state: &IssuesState) -> Self {
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
    issues: Issues,
    /// Shortcut widget
    shortcuts: Shortcuts<Action>,
}

impl Widget<IssuesState, Action> for ListPage {
    fn new(state: &IssuesState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
            props: ListPageProps::from(state),
            issues: Issues::new(state, action_tx.clone()),
            shortcuts: Shortcuts::new(state, action_tx.clone()),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &IssuesState) -> Self
    where
        Self: Sized,
    {
        ListPage {
            issues: self.issues.move_with_state(state),
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
                        Mode::Operation => Some(IssueOperation::Show.to_string()),
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
                            operation: Some(IssueOperation::Comment.to_string()),
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
                            operation: Some(IssueOperation::Edit.to_string()),
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
                            operation: Some(IssueOperation::Delete.to_string()),
                            ids: vec![selected.id],
                            args: vec![],
                        }),
                    });
                }
            }
            _ => {
                <Issues as Widget<IssuesState, Action>>::handle_key_event(&mut self.issues, key);
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
                Shortcut::new("c", "comment"),
                Shortcut::new("e", "edit"),
                Shortcut::new("d", "delete"),
                Shortcut::new("q", "quit"),
            ],
        };

        self.issues.render::<B>(frame, layout.component, ());
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

struct IssuesProps {
    issues: Vec<IssueItem>,
    filter: Filter,
    stats: HashMap<String, usize>,
}

impl From<&IssuesState> for IssuesProps {
    fn from(state: &IssuesState) -> Self {
        let mut open = 0;
        let mut closed = 0;

        for issue in &state.issues {
            match issue.state {
                issue::State::Open => open += 1,
                issue::State::Closed { reason: _ } => closed += 1,
            }
        }
        let stats = HashMap::from([("Open".to_string(), open), ("Closed".to_string(), closed)]);

        Self {
            issues: state.issues.clone(),
            filter: state.filter.clone(),
            stats,
        }
    }
}

struct Issues {
    /// Action sender
    action_tx: UnboundedSender<Action>,
    /// State mapped props
    props: IssuesProps,
    /// Header
    header: Header<Action>,
    /// Notification table
    table: Table<Action>,
    /// Footer
    footer: Footer<Action>,
}

impl Widget<IssuesState, Action> for Issues {
    fn new(state: &IssuesState, action_tx: UnboundedSender<Action>) -> Self {
        Self {
            action_tx: action_tx.clone(),
            props: IssuesProps::from(state),
            header: Header::new(state, action_tx.clone()),
            table: Table::new(state, action_tx.clone()),
            footer: Footer::new(state, action_tx),
        }
    }

    fn move_with_state(self, state: &IssuesState) -> Self
    where
        Self: Sized,
    {
        Self {
            props: IssuesProps::from(state),
            table: self.table.move_with_state(state),
            header: self.header.move_with_state(state),
            footer: self.footer.move_with_state(state),
            ..self
        }
    }

    fn name(&self) -> &str {
        "issues"
    }

    fn handle_key_event(&mut self, key: Key) {
        match key {
            Key::Up => {
                self.table.prev();

                let selected = self
                    .table
                    .selected()
                    .and_then(|selected| self.props.issues.get(selected));

                // TODO: propagate error
                if let Some(notif) = selected {
                    let _ = self.action_tx.send(Action::Select {
                        item: notif.clone(),
                    });
                }
            }
            Key::Down => {
                self.table.next(self.props.issues.len());

                let selected = self
                    .table
                    .selected()
                    .and_then(|selected| self.props.issues.get(selected));

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

impl Render<()> for Issues {
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
            Constraint::Fill(5),
            Constraint::Length(16),
            Constraint::Length(16),
            Constraint::Fill(1),
            Constraint::Fill(1),
            Constraint::Length(16),
        ];

        let filter = if !self.props.filter.to_string().is_empty() {
            Line::from(
                [
                    span::badge("▼".to_string()),
                    span::default(" ".to_string()),
                    span::default(self.props.filter.to_string()).magenta().dim(),
                ]
                .to_vec(),
            )
        } else {
            Line::from([span::blank()].to_vec())
        };

        let stats = Line::from(
            [
                span::positive(self.props.stats.get("Open").unwrap_or(&0).to_string()).dim(),
                span::default(" Open".to_string()).dim(),
                span::default(" | ".to_string()).dim(),
                span::default(self.props.stats.get("Closed").unwrap_or(&0).to_string())
                    .magenta()
                    .dim(),
                span::default(" Closed".to_string()).dim(),
            ]
            .to_vec(),
        )
        .alignment(Alignment::Right);

        let (step, len) = self.table.progress(self.props.issues.len());
        let progress = span::progress(step, len, false);

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
                    String::from("Labels").into(),
                    String::from("Assignees ").into(),
                    String::from("Opened").into(),
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
                items: self.props.issues.to_vec(),
                has_footer: true,
                has_header: true,
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
