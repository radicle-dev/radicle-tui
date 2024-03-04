use std::collections::HashMap;
use std::vec;

use radicle::issue;
use ratatui::style::Stylize;
use ratatui::text::Line;
use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

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
            Key::Esc | Key::Ctrl('c') => {
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
            Mode::Id => vec![Shortcut::new("enter", "select")],
            Mode::Operation => vec![
                Shortcut::new("enter", "show"),
                Shortcut::new("c", "comment"),
                Shortcut::new("e", "edit"),
                Shortcut::new("d", "delete"),
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
    widths: [Constraint; 8],
    cutoff: usize,
    cutoff_after: usize,
    focus: bool,
    page_size: usize,
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
            widths: [
                Constraint::Length(3),
                Constraint::Length(8),
                Constraint::Fill(5),
                Constraint::Length(16),
                Constraint::Length(16),
                Constraint::Fill(1),
                Constraint::Fill(1),
                Constraint::Length(16),
            ],
            cutoff: 200,
            cutoff_after: 5,
            focus: false,
            stats,
            page_size: state.ui.page_size,
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
            Key::Up | Key::Char('k') => {
                self.table.prev();
            }
            Key::Down | Key::Char('j') => {
                self.table.next(self.props.issues.len());
            }
            Key::PageUp => {
                self.table.prev_page(self.props.page_size);
            }
            Key::PageDown => {
                self.table
                    .next_page(self.props.issues.len(), self.props.page_size);
            }
            Key::Home => {
                self.table.begin();
            }
            Key::End => {
                self.table.end(self.props.issues.len());
            }
            _ => {}
        }
        self.table
            .selected()
            .and_then(|selected| self.props.issues.get(selected))
            .and_then(|issue| {
                self.action_tx
                    .send(Action::Select {
                        item: issue.clone(),
                    })
                    .ok()
            });
    }
}

impl Issues {
    fn render_header<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect) {
        self.header.render::<B>(
            frame,
            area,
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
                widths: self.props.widths,
                focus: self.props.focus,
                cutoff: self.props.cutoff,
                cutoff_after: self.props.cutoff_after,
            },
        );
    }

    fn render_list<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect) {
        self.table.render::<B>(
            frame,
            area,
            TableProps {
                items: self.props.issues.to_vec(),
                has_footer: true,
                has_header: true,
                widths: self.props.widths,
                focus: self.props.focus,
                cutoff: self.props.cutoff,
                cutoff_after: self.props.cutoff_after,
            },
        );
    }

    fn render_footer<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect) {
        let filter = Line::from(
            [
                span::default(" ".to_string()),
                span::default(self.props.filter.to_string()).magenta().dim(),
            ]
            .to_vec(),
        );
        let open = Line::from(
            [
                span::positive(self.props.stats.get("Open").unwrap_or(&0).to_string()).dim(),
                span::default(" Open".to_string()).dim(),
            ]
            .to_vec(),
        );
        let closed = Line::from(
            [
                span::default(self.props.stats.get("Closed").unwrap_or(&0).to_string())
                    .magenta()
                    .dim(),
                span::default(" Closed".to_string()).dim(),
            ]
            .to_vec(),
        );
        let sum = Line::from(
            [
                span::default("Σ ".to_string()).dim(),
                span::default(self.props.issues.len().to_string()).dim(),
            ]
            .to_vec(),
        );

        let progress = self
            .table
            .progress_percentage(self.props.issues.len(), self.props.page_size);
        let progress = span::default(format!("{}%", progress)).dim();

        self.footer.render::<B>(
            frame,
            area,
            FooterProps {
                cells: [
                    filter.into(),
                    open.clone().into(),
                    closed.clone().into(),
                    sum.clone().into(),
                    progress.clone().into(),
                ],
                widths: [
                    Constraint::Fill(1),
                    Constraint::Min(open.width() as u16),
                    Constraint::Min(closed.width() as u16),
                    Constraint::Min(sum.width() as u16),
                    Constraint::Min(4),
                ],
                focus: self.props.focus,
                cutoff: self.props.cutoff,
                cutoff_after: self.props.cutoff_after,
            },
        );
    }
}

impl Render<()> for Issues {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(area);

        self.render_header::<B>(frame, layout[0]);
        self.render_list::<B>(frame, layout[1]);
        self.render_footer::<B>(frame, layout[2]);

        let page_size = layout[1].height as usize;
        if page_size != self.props.page_size {
            let _ = self
                .action_tx
                .send(Action::PageSize(layout[1].height as usize));
        }
    }
}
