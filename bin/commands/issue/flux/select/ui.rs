use std::collections::HashMap;
use std::str::FromStr;
use std::vec;

use radicle::issue::{self, CloseReason};
use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;

use radicle_tui as tui;

use tui::flux::ui::cob::{IssueItem, IssueItemFilter};
use tui::flux::ui::span;
use tui::flux::ui::widget::container::{Footer, FooterProps, Header, HeaderProps};
use tui::flux::ui::widget::input::{TextField, TextFieldProps};
use tui::flux::ui::widget::{
    Render, Shortcut, Shortcuts, ShortcutsProps, Table, TableProps, Widget,
};
use tui::Selection;

use crate::tui_issue::common::IssueOperation;
use crate::tui_issue::common::Mode;

use super::{Action, State};

pub struct ListPageProps {
    selected: Option<IssueItem>,
    mode: Mode,
    show_search: bool,
}

impl From<&State> for ListPageProps {
    fn from(state: &State) -> Self {
        Self {
            selected: state.selected.clone(),
            mode: state.mode.clone(),
            show_search: state.ui.show_search,
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
    /// Search widget
    search: Search,
    /// Shortcut widget
    shortcuts: Shortcuts<Action>,
}

impl Widget<State, Action> for ListPage {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
            props: ListPageProps::from(state),
            issues: Issues::new(state, action_tx.clone()),
            search: Search::new(state, action_tx.clone()),
            shortcuts: Shortcuts::new(state, action_tx.clone()),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &State) -> Self
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
        if self.props.show_search {
            <Search as Widget<State, Action>>::handle_key_event(&mut self.search, key)
        } else {
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
                Key::Char('/') => {
                    let _ = self.action_tx.send(Action::OpenSearch);
                }
                _ => {
                    <Issues as Widget<State, Action>>::handle_key_event(&mut self.issues, key);
                }
            }
        }
    }
}

impl Render<()> for ListPage {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, _area: Rect, _props: ()) {
        let area = frame.size();
        let layout = tui::flux::ui::layout::default_page(area, 0u16, 1u16);

        let shortcuts = if self.props.show_search {
            vec![
                Shortcut::new("esc", "back"),
                Shortcut::new("enter", "search"),
            ]
        } else {
            match self.props.mode {
                Mode::Id => vec![
                    Shortcut::new("enter", "select"),
                    Shortcut::new("/", "search"),
                ],
                Mode::Operation => vec![
                    Shortcut::new("enter", "show"),
                    Shortcut::new("e", "edit"),
                    Shortcut::new("/", "search"),
                ],
            }
        };

        if self.props.show_search {
            let component_layout = Layout::vertical([Constraint::Min(1), Constraint::Length(2)])
                .split(layout.component);

            self.issues.render::<B>(frame, component_layout[0], ());
            self.search
                .render::<B>(frame, component_layout[1], SearchProps {});
        } else {
            self.issues.render::<B>(frame, layout.component, ());
        }

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
    search: String,
    stats: HashMap<String, usize>,
    widths: [Constraint; 8],
    cutoff: usize,
    cutoff_after: usize,
    focus: bool,
    page_size: usize,
    show_search: bool,
}

impl From<&State> for IssuesProps {
    fn from(state: &State) -> Self {
        use radicle::issue::State;

        let mut open = 0;
        let mut other = 0;
        let mut solved = 0;

        // Filter by search string
        let filter = IssueItemFilter::from_str(&state.search.read()).unwrap_or_default();
        let mut issues = state
            .issues
            .clone()
            .into_iter()
            .filter(|issue| filter.matches(issue))
            .collect::<Vec<_>>();

        // Apply sorting
        issues.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        for issue in &state.issues {
            match issue.state {
                State::Open => open += 1,
                State::Closed {
                    reason: CloseReason::Other,
                } => other += 1,
                State::Closed {
                    reason: CloseReason::Solved,
                } => solved += 1,
            }
        }

        let closed = solved + other;

        let stats = HashMap::from([
            ("Open".to_string(), open),
            ("Other".to_string(), other),
            ("Solved".to_string(), solved),
            ("Closed".to_string(), closed),
        ]);

        Self {
            issues,
            search: state.search.read(),
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
            show_search: state.ui.show_search,
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

impl Widget<State, Action> for Issues {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self {
        Self {
            action_tx: action_tx.clone(),
            props: IssuesProps::from(state),
            header: Header::new(state, action_tx.clone()),
            table: Table::new(state, action_tx.clone()),
            footer: Footer::new(state, action_tx),
        }
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        let props = IssuesProps::from(state);
        let mut table = self.table.move_with_state(state);

        if let Some(selected) = table.selected() {
            if selected > props.issues.len() {
                table.begin();
            }
        }

        Self {
            props,
            table,
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
                has_footer: !self.props.show_search,
                has_header: true,
                widths: self.props.widths,
                focus: self.props.focus,
                cutoff: self.props.cutoff,
                cutoff_after: self.props.cutoff_after,
            },
        );
    }

    fn render_footer<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect) {
        let search = if self.props.search.is_empty() {
            Line::from([span::default(self.props.search.to_string()).magenta().dim()].to_vec())
        } else {
            Line::from(
                [
                    span::default(" / ".to_string()).magenta().dim(),
                    span::default(self.props.search.to_string()).magenta().dim(),
                ]
                .to_vec(),
            )
        };

        let open = Line::from(
            [
                span::positive(self.props.stats.get("Open").unwrap_or(&0).to_string()).dim(),
                span::default(" Open".to_string()).dim(),
            ]
            .to_vec(),
        );
        let solved = Line::from(
            [
                span::default(self.props.stats.get("Solved").unwrap_or(&0).to_string())
                    .magenta()
                    .dim(),
                span::default(" Solved".to_string()).dim(),
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

        match IssueItemFilter::from_str(&self.props.search)
            .unwrap_or_default()
            .state()
        {
            Some(state) => {
                let block = match state {
                    issue::State::Open => open,
                    issue::State::Closed {
                        reason: issue::CloseReason::Other,
                    } => closed,
                    issue::State::Closed {
                        reason: issue::CloseReason::Solved,
                    } => solved,
                };

                self.footer.render::<B>(
                    frame,
                    area,
                    FooterProps {
                        cells: [search.into(), block.clone().into(), progress.clone().into()],
                        widths: [
                            Constraint::Fill(1),
                            Constraint::Min(block.width() as u16),
                            Constraint::Min(4),
                        ],
                        focus: self.props.focus,
                        cutoff: self.props.cutoff,
                        cutoff_after: self.props.cutoff_after,
                    },
                );
            }
            None => {
                self.footer.render::<B>(
                    frame,
                    area,
                    FooterProps {
                        cells: [
                            search.into(),
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
    }
}

impl Render<()> for Issues {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let page_size = if self.props.show_search {
            let layout = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(area);

            self.render_header::<B>(frame, layout[0]);
            self.render_list::<B>(frame, layout[1]);

            layout[1].height as usize
        } else {
            let layout = Layout::vertical([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(area);

            self.render_header::<B>(frame, layout[0]);
            self.render_list::<B>(frame, layout[1]);
            self.render_footer::<B>(frame, layout[2]);

            layout[1].height as usize
        };

        if page_size != self.props.page_size {
            let _ = self.action_tx.send(Action::PageSize(page_size));
        }
    }
}

pub struct SearchProps {}

pub struct Search {
    pub action_tx: UnboundedSender<Action>,
    pub input: TextField,
}

impl Widget<State, Action> for Search {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let mut input = TextField::new(state, action_tx.clone());
        input.set_text(&state.search.read().to_string());

        Self { action_tx, input }.move_with_state(state)
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        let mut input = <TextField as Widget<State, Action>>::move_with_state(self.input, state);
        input.set_text(&state.search.read().to_string());

        Self { input, ..self }
    }

    fn name(&self) -> &str {
        "filter-popup"
    }

    fn handle_key_event(&mut self, key: termion::event::Key) {
        match key {
            Key::Esc => {
                let _ = self.action_tx.send(Action::CloseSearch);
            }
            Key::Char('\n') => {
                let _ = self.action_tx.send(Action::ApplySearch);
            }
            _ => {
                <TextField as Widget<State, Action>>::handle_key_event(&mut self.input, key);
                let _ = self.action_tx.send(Action::UpdateSearch {
                    value: self.input.text().to_string(),
                });
            }
        }
    }
}

impl Render<SearchProps> for Search {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, _props: SearchProps) {
        let layout = Layout::horizontal(Constraint::from_mins([0]))
            .horizontal_margin(1)
            .split(area);

        self.input.render::<B>(
            frame,
            layout[0],
            TextFieldProps {
                titles: ("/".into(), "Search".into()),
                show_cursor: true,
                inline_label: true,
            },
        );
    }
}
