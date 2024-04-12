use std::collections::HashMap;
use std::str::FromStr;
use std::vec;

use radicle::issue::{self, CloseReason};
use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};

use radicle_tui as tui;

use tui::ui::items::{Filter, IssueItem, IssueItemFilter};
use tui::ui::span;
use tui::ui::widget::container::{Footer, Header};
use tui::ui::widget::input::TextField;
use tui::ui::widget::text::Paragraph;
use tui::ui::widget::{Column, Render, Shortcuts, Table, View};
use tui::Selection;

use crate::tui_issue::common::IssueOperation;
use crate::tui_issue::common::Mode;

use super::{Action, State};

pub struct ListPageProps {
    show_search: bool,
    show_help: bool,
}

impl From<&State> for ListPageProps {
    fn from(state: &State) -> Self {
        Self {
            show_search: state.ui.show_search,
            show_help: state.ui.show_help,
        }
    }
}

pub struct ListPage<'a, B: Backend> {
    /// Action sender
    pub action_tx: UnboundedSender<Action>,
    /// State mapped props
    props: ListPageProps,
    /// Notification widget
    issues: Issues<'a>,
    /// Search widget
    search: Search,
    /// Help widget
    help: Help<'a>,
    /// Shortcut widget
    shortcuts: Shortcuts<State, Action, B>,
}

impl<'a, B: Backend> View<State, Action> for ListPage<'a, B> {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
            props: ListPageProps::from(state),
            issues: Issues::new(state, action_tx.clone()),
            search: Search::new(state, action_tx.clone()),
            help: Help::new(state, action_tx.clone()),
            shortcuts: Shortcuts::new(state, action_tx.clone()),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        let shorts = if state.ui.show_search {
            vec![("esc", "cancel"), ("enter", "apply")]
        } else if state.ui.show_help {
            vec![("?", "close")]
        } else {
            match state.mode {
                Mode::Id => vec![("enter", "select"), ("/", "search")],
                Mode::Operation => vec![
                    ("enter", "show"),
                    ("e", "edit"),
                    ("/", "search"),
                    ("?", "help"),
                ],
            }
        };

        let shortcuts = self.shortcuts.move_with_state(state);
        let shortcuts = shortcuts.shortcuts(&shorts);

        ListPage {
            issues: self.issues.move_with_state(state),
            shortcuts,
            help: self.help.move_with_state(state),
            props: ListPageProps::from(state),
            ..self
        }
    }

    fn update(&mut self, state: &State) {}

    fn handle_key_event(&mut self, key: termion::event::Key) {
        if self.props.show_search {
            <Search as View<State, Action>>::handle_key_event(&mut self.search, key)
        } else if self.props.show_help {
            <Help as View<State, Action>>::handle_key_event(&mut self.help, key)
        } else {
            match key {
                Key::Esc | Key::Ctrl('c') => {
                    let _ = self.action_tx.send(Action::Exit { selection: None });
                }
                Key::Char('/') => {
                    let _ = self.action_tx.send(Action::OpenSearch);
                }
                Key::Char('?') => {
                    let _ = self.action_tx.send(Action::OpenHelp);
                }
                _ => {
                    <Issues as View<State, Action>>::handle_key_event(&mut self.issues, key);
                }
            }
        }
        let _ = self.action_tx.send(Action::Update);
    }
}

impl<'a, B: Backend> Render<B, ()> for ListPage<'a, B> {
    fn render(&self, frame: &mut ratatui::Frame, _area: Rect, _props: ()) {
        let area = frame.size();
        let layout = tui::ui::layout::default_page(area, 0u16, 1u16);

        if self.props.show_search {
            let component_layout = Layout::vertical([Constraint::Min(1), Constraint::Length(2)])
                .split(layout.component);

            <Issues<'_> as Render<B, ()>>::render(&self.issues, frame, component_layout[0], ());
            <Search as Render<B, ()>>::render(&self.search, frame, component_layout[1], ());
        } else if self.props.show_help {
            <Help<'_> as Render<B, ()>>::render(&self.help, frame, layout.component, ());
        } else {
            <Issues<'_> as Render<B, ()>>::render(&self.issues, frame, layout.component, ());
        }

        <Shortcuts<_, _, B> as Render<B, ()>>::render(&self.shortcuts, frame, layout.shortcuts, ());
    }
}

#[derive(Clone)]
struct IssuesProps<'a> {
    mode: Mode,
    issues: Vec<IssueItem>,
    search: String,
    stats: HashMap<String, usize>,
    columns: Vec<Column<'a>>,
    cutoff: usize,
    cutoff_after: usize,
    focus: bool,
    page_size: usize,
    show_search: bool,
}

impl<'a> From<&State> for IssuesProps<'a> {
    fn from(state: &State) -> Self {
        use radicle::issue::State;

        let issues: Vec<IssueItem> = state
            .issues
            .iter()
            .filter(|issue| state.filter.matches(issue))
            .cloned()
            .collect();

        let mut open = 0;
        let mut other = 0;
        let mut solved = 0;

        for issue in &issues {
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
            mode: state.mode.clone(),
            issues,
            search: state.search.read(),
            columns: [
                Column::new(" ● ", Constraint::Length(3)),
                Column::new("ID", Constraint::Length(8)),
                Column::new("Title", Constraint::Fill(5)),
                Column::new("Author", Constraint::Length(16)),
                Column::new("", Constraint::Length(16)),
                Column::new("Labels", Constraint::Fill(1)),
                Column::new("Assignees", Constraint::Fill(1)),
                Column::new("Opened", Constraint::Length(16)),
            ]
            .to_vec(),
            cutoff: 200,
            cutoff_after: 5,
            focus: false,
            stats,
            page_size: state.ui.page_size,
            show_search: state.ui.show_search,
        }
    }
}

struct Issues<'a> {
    /// Action sender
    action_tx: UnboundedSender<Action>,
    /// State mapped props
    props: IssuesProps<'a>,
    /// Notification table
    table: Table<'a, Action, IssueItem>,
    /// Footer
    footer: Footer<'a, Action>,
}

impl<'a> View<State, Action> for Issues<'a> {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self {
        let props = IssuesProps::from(state);

        Self {
            action_tx: action_tx.clone(),
            props: props.clone(),
            table: Table::new(&(), action_tx.clone())
                .items(props.issues.clone())
                .columns(props.columns.to_vec())
                .header(
                    Header::new(&(), action_tx.clone())
                        .columns(props.columns.clone())
                        .cutoff(props.cutoff, props.cutoff_after)
                        .focus(props.focus),
                )
                .footer(!props.show_search)
                .cutoff(props.cutoff, props.cutoff_after),
            footer: Footer::new(&(), action_tx),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        let issues: Vec<IssueItem> = state
            .issues
            .iter()
            .filter(|issue| state.filter.matches(issue))
            .cloned()
            .collect();

        let props = IssuesProps::from(state);

        let table = self.table.move_with_state(&());
        let table = table
            .items(issues)
            .footer(!state.ui.show_search)
            .page_size(state.ui.page_size);

        let footer = self.footer.move_with_state(&());
        let footer = footer.columns(Self::build_footer(&props, table.selected()));

        Self {
            props,
            table,
            footer,
            ..self
        }
    }

    fn update(&mut self, state: &State) {}

    fn handle_key_event(&mut self, key: Key) {
        match key {
            Key::Char('\n') => {
                let operation = match self.props.mode {
                    Mode::Operation => Some(IssueOperation::Show.to_string()),
                    Mode::Id => None,
                };

                self.table
                    .selected()
                    .and_then(|selected| self.props.issues.get(selected))
                    .and_then(|issue| {
                        self.action_tx
                            .send(Action::Exit {
                                selection: Some(Selection {
                                    operation,
                                    ids: vec![issue.id],
                                    args: vec![],
                                }),
                            })
                            .ok()
                    });
            }
            Key::Char('e') => {
                self.table
                    .selected()
                    .and_then(|selected| self.props.issues.get(selected))
                    .and_then(|issue| {
                        self.action_tx
                            .send(Action::Exit {
                                selection: Some(Selection {
                                    operation: Some(IssueOperation::Edit.to_string()),
                                    ids: vec![issue.id],
                                    args: vec![],
                                }),
                            })
                            .ok()
                    });
            }
            _ => {
                <Table<Action, IssueItem> as View<(), Action>>::handle_key_event(
                    &mut self.table,
                    key,
                );
            }
        }
    }
}

impl<'a> Issues<'a> {
    fn build_footer(props: &IssuesProps<'a>, selected: Option<usize>) -> Vec<Column<'a>> {
        let search = Line::from(
            [
                span::default(" Search ".to_string())
                    .cyan()
                    .dim()
                    .reversed(),
                span::default(" ".into()),
                span::default(props.search.to_string()).gray().dim(),
            ]
            .to_vec(),
        );

        let open = Line::from(
            [
                span::positive(props.stats.get("Open").unwrap_or(&0).to_string()).dim(),
                span::default(" Open".to_string()).dim(),
            ]
            .to_vec(),
        );
        let solved = Line::from(
            [
                span::default(props.stats.get("Solved").unwrap_or(&0).to_string())
                    .magenta()
                    .dim(),
                span::default(" Solved".to_string()).dim(),
            ]
            .to_vec(),
        );
        let closed = Line::from(
            [
                span::default(props.stats.get("Closed").unwrap_or(&0).to_string())
                    .magenta()
                    .dim(),
                span::default(" Closed".to_string()).dim(),
            ]
            .to_vec(),
        );
        let sum = Line::from(
            [
                span::default("Σ ".to_string()).dim(),
                span::default(props.issues.len().to_string()).dim(),
            ]
            .to_vec(),
        );

        let progress = selected
            .map(|selected| {
                Table::<Action, IssueItem>::progress(selected, props.issues.len(), props.page_size)
            })
            .unwrap_or_default();
        let progress = span::default(format!("{}%", progress)).dim();

        match IssueItemFilter::from_str(&props.search)
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

                [
                    Column::new(Text::from(search), Constraint::Fill(1)),
                    Column::new(
                        Text::from(block.clone()),
                        Constraint::Min(block.width() as u16),
                    ),
                    Column::new(Text::from(progress), Constraint::Min(4)),
                ]
                .to_vec()
            }
            None => [
                Column::new(Text::from(search), Constraint::Fill(1)),
                Column::new(
                    Text::from(open.clone()),
                    Constraint::Min(open.width() as u16),
                ),
                Column::new(
                    Text::from(closed.clone()),
                    Constraint::Min(closed.width() as u16),
                ),
                Column::new(Text::from(sum.clone()), Constraint::Min(sum.width() as u16)),
                Column::new(Text::from(progress), Constraint::Min(4)),
            ]
            .to_vec(),
        }
    }
}

impl<'a, B: Backend> Render<B, ()> for Issues<'a> {
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let header_height = 3_usize;

        let page_size = if self.props.show_search {
            <Table<'_, _, _> as Render<B, ()>>::render(&self.table, frame, area, ());

            (area.height as usize).saturating_sub(header_height)
        } else {
            let layout = Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(area);

            <Table<'_, _, _> as Render<B, ()>>::render(&self.table, frame, layout[0], ());
            <Footer<'_, _> as Render<B, ()>>::render(&self.footer, frame, layout[1], ());

            (area.height as usize).saturating_sub(header_height)
        };

        if page_size != self.props.page_size {
            let _ = self.action_tx.send(Action::PageSize(page_size));
        }
    }
}

pub struct Search {
    pub action_tx: UnboundedSender<Action>,
    pub input: TextField<Action>,
}

impl View<State, Action> for Search {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let input = TextField::new(state, action_tx.clone())
            .title("Search")
            .inline(true);
        Self { action_tx, input }.move_with_state(state)
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        let input = self.input.move_with_state(state);
        let input = input.text(&state.search.read().to_string());

        Self { input, ..self }
    }

    fn update(&mut self, state: &State) {}

    fn handle_key_event(&mut self, key: termion::event::Key) {
        match key {
            Key::Esc => {
                let _ = self.action_tx.send(Action::CloseSearch);
            }
            Key::Char('\n') => {
                let _ = self.action_tx.send(Action::ApplySearch);
            }
            _ => {
                <TextField<Action> as View<State, Action>>::handle_key_event(&mut self.input, key);
                let _ = self.action_tx.send(Action::UpdateSearch {
                    value: self.input.read().to_string(),
                });
            }
        }
    }
}

impl<B: Backend> Render<B, ()> for Search {
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let layout = Layout::horizontal(Constraint::from_mins([0]))
            .horizontal_margin(1)
            .split(area);

        <TextField<_> as Render<B, ()>>::render(&self.input, frame, layout[0], ());
    }
}

pub struct HelpProps<'a> {
    content: Text<'a>,
    focus: bool,
    page_size: usize,
}

impl<'a> From<&State> for HelpProps<'a> {
    fn from(state: &State) -> Self {
        let content = Text::from(
            [
                Line::from(Span::raw("Generic keybindings").cyan()),
                Line::raw(""),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "↑,k")).gray(),
                        Span::raw(" "),
                        Span::raw("move cursor one line up").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "↓,j")).gray(),
                        Span::raw(" "),
                        Span::raw("move cursor one line down").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "PageUp")).gray(),
                        Span::raw(" "),
                        Span::raw("move cursor one page up").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "PageDown")).gray(),
                        Span::raw(" "),
                        Span::raw("move cursor one page down").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "Home")).gray(),
                        Span::raw(" "),
                        Span::raw("move cursor to the first line").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "End")).gray(),
                        Span::raw(" "),
                        Span::raw("move cursor to the last line").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::raw(""),
                Line::from(Span::raw("Specific keybindings").cyan()),
                Line::raw(""),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "enter")).gray(),
                        Span::raw(" "),
                        Span::raw("Select issue (if --mode id)").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "enter")).gray(),
                        Span::raw(" "),
                        Span::raw("Show issue").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "e")).gray(),
                        Span::raw(" "),
                        Span::raw("Edit patch").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "/")).gray(),
                        Span::raw(" "),
                        Span::raw("Search").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "?")).gray(),
                        Span::raw(" "),
                        Span::raw("Show help").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "Esc")).gray(),
                        Span::raw(" "),
                        Span::raw("Quit / cancel").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::raw(""),
                Line::from(Span::raw("Searching").cyan()),
                Line::raw(""),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "Pattern")).gray(),
                        Span::raw(" "),
                        Span::raw("is:<state> | is:authored | is:assigned | authors:[<did>, ...] | assignees:[<did>, ...] | <search>")
                            .gray()
                            .dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "Example")).gray(),
                        Span::raw(" "),
                        Span::raw("is:solved is:authored alias").gray().dim(),
                    ]
                    .to_vec(),
                ),
            ]
            .to_vec(),
        );

        Self {
            content,
            focus: false,
            page_size: state.ui.page_size,
        }
    }
}

pub struct Help<'a> {
    /// Send messages
    pub action_tx: UnboundedSender<Action>,
    /// This widget's render properties
    pub props: HelpProps<'a>,
    /// Container header
    header: Header<'a, Action>,
    /// Content widget
    content: Paragraph<'a, Action>,
    /// Container footer
    footer: Footer<'a, Action>,
}

impl<'a> View<State, Action> for Help<'a> {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let props = HelpProps::from(state);

        Self {
            action_tx: action_tx.clone(),
            props,
            header: Header::new(&(), action_tx.clone()),
            content: Paragraph::new(state, action_tx.clone()),
            footer: Footer::new(&(), action_tx),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        let props = HelpProps::from(state);

        let header = self.header.move_with_state(&());
        let header = header
            .columns([Column::new(" Help ", Constraint::Fill(1))].to_vec())
            .focus(props.focus);

        let content = self.content.move_with_state(state);
        let content = content.text(&props.content).page_size(props.page_size);

        let progress = span::default(format!("{}%", content.progress())).dim();

        let footer = self.footer.move_with_state(&());
        let footer = footer
            .columns(
                [
                    Column::new(Text::raw(""), Constraint::Fill(1)),
                    Column::new(Text::from(progress), Constraint::Min(4)),
                ]
                .to_vec(),
            )
            .focus(props.focus);

        Self {
            props,
            header,
            content,
            footer,
            ..self
        }
    }

    fn update(&mut self, state: &State) {}

    fn handle_key_event(&mut self, key: termion::event::Key) {
        match key {
            Key::Esc => {
                let _ = self.action_tx.send(Action::Exit { selection: None });
            }
            Key::Char('?') => {
                let _ = self.action_tx.send(Action::CloseHelp);
            }
            _ => {
                <Paragraph<_> as View<(), _>>::handle_key_event(&mut self.content, key);
            }
        }
    }
}

impl<'a, B: Backend> Render<B, ()> for Help<'a> {
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let [header_area, content_area, footer_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .areas(area);

        <Header<'_, _> as Render<B, ()>>::render(&self.header, frame, header_area, ());
        <Paragraph<'_, _> as Render<B, ()>>::render(&self.content, frame, content_area, ());
        <Footer<'_, _> as Render<B, ()>>::render(&self.footer, frame, footer_area, ());

        let page_size = content_area.height as usize;
        if page_size != self.props.page_size {
            let _ = self.action_tx.send(Action::PageSize(page_size));
        }
    }
}
