use std::any::Any;
use std::collections::HashMap;
use std::str::FromStr;
use std::vec;

use radicle::issue::{self, CloseReason};
use ratatui::widgets::TableState;
use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};

use radicle_tui as tui;

use tui::ui::items::{IssueItem, IssueItemFilter};
use tui::ui::span;
use tui::ui::widget;
use tui::ui::widget::container::{Footer, FooterProps, Header, HeaderProps};
use tui::ui::widget::input::{TextField, TextFieldProps, TextFieldState};
use tui::ui::widget::text::{Paragraph, ParagraphProps, ParagraphState};
use tui::ui::widget::{
    Column, EventCallback, Properties, Shortcuts, ShortcutsProps, Table, TableProps, TableUtils,
    UpdateCallback, View, Widget,
};
use tui::Selection;

use crate::tui_issue::common::IssueOperation;
use crate::tui_issue::common::Mode;

use super::{Action, State};

type BoxedWidget<B> = widget::BoxedWidget<B, State, Action>;

pub struct ListPageProps {
    show_search: bool,
    show_help: bool,
}

impl From<&State> for ListPageProps {
    fn from(state: &State) -> Self {
        Self {
            show_search: state.ui.show_search,
            show_help: state.help.show,
        }
    }
}

pub struct ListPage<B: Backend> {
    /// Internal properties
    props: ListPageProps,
    /// Message sender
    action_tx: UnboundedSender<Action>,
    /// Custom update handler
    on_update: Option<UpdateCallback<State>>,
    /// Additional custom event handler
    on_change: Option<EventCallback<Action>>,
    /// Patches widget
    issues: BoxedWidget<B>,
    /// Search widget
    search: BoxedWidget<B>,
    /// Help widget
    help: BoxedWidget<B>,
    /// Shortcut widget
    shortcuts: BoxedWidget<B>,
}

impl<'a: 'static, B> View<State, Action> for ListPage<B>
where
    B: Backend + 'a,
{
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
            props: ListPageProps::from(state),
            issues: Issues::new(state, action_tx.clone()).to_boxed(),
            search: Search::new(state, action_tx.clone()).to_boxed(),
            help: Help::new(state, action_tx.clone()).to_boxed(),
            shortcuts: Shortcuts::new(state, action_tx.clone())
                .on_update(|state| {
                    ShortcutsProps::default()
                        .shortcuts(&state.shortcuts())
                        .to_boxed()
                })
                .to_boxed(),
            on_update: None,
            on_change: None,
        }
    }

    fn on_change(mut self, callback: EventCallback<Action>) -> Self {
        self.on_change = Some(callback);
        self
    }

    fn on_update(mut self, callback: UpdateCallback<State>) -> Self {
        self.on_update = Some(callback);
        self
    }

    fn update(&mut self, state: &State) {
        self.props = ListPageProps::from(state);

        self.issues.update(state);
        self.search.update(state);
        self.help.update(state);
        self.shortcuts.update(state);
    }

    fn handle_key_event(&mut self, key: termion::event::Key) {
        if self.props.show_search {
            self.search.handle_key_event(key);
        } else if self.props.show_help {
            self.help.handle_key_event(key);
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
                    self.issues.handle_key_event(key);
                }
            }
        }
    }
}

impl<'a: 'static, B> Widget<B, State, Action> for ListPage<B>
where
    B: Backend + 'a,
{
    fn render(&self, frame: &mut ratatui::Frame, _area: Rect, _props: Option<&dyn Any>) {
        let area = frame.size();
        let layout = tui::ui::layout::default_page(area, 0u16, 1u16);

        if self.props.show_search {
            let component_layout = Layout::vertical([Constraint::Min(1), Constraint::Length(2)])
                .split(layout.component);

            self.issues.render(frame, component_layout[0], None);
            self.search.render(frame, component_layout[1], None);
        } else if self.props.show_help {
            self.help.render(frame, layout.component, None);
        } else {
            self.issues.render(frame, layout.component, None);
        }

        self.shortcuts.render(frame, layout.shortcuts, None);
    }
}

#[derive(Clone)]
struct IssuesProps<'a> {
    mode: Mode,
    issues: Vec<IssueItem>,
    selected: Option<usize>,
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

        let issues = state.issues();

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
            selected: state.issues.selected,
        }
    }
}

struct Issues<'a, B> {
    /// Internal properties
    props: IssuesProps<'a>,
    /// Message sender
    action_tx: UnboundedSender<Action>,
    /// Custom update handler
    on_update: Option<UpdateCallback<State>>,
    /// Additional custom event handler
    on_change: Option<EventCallback<Action>>,
    /// Table widget
    table: BoxedWidget<B>,
    /// Footer widget w/ context
    footer: BoxedWidget<B>,
}

impl<'a: 'static, B> View<State, Action> for Issues<'a, B>
where
    B: Backend + 'a,
{
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self {
        let props = IssuesProps::from(state);

        Self {
            action_tx: action_tx.clone(),
            props: props.clone(),
            table: Box::<Table<B, State, Action, IssueItem>>::new(
                Table::new(state, action_tx.clone())
                    .header(
                        Header::new(state, action_tx.clone())
                            .columns(props.columns.clone())
                            .cutoff(props.cutoff, props.cutoff_after)
                            .focus(props.focus)
                            .to_boxed(),
                    )
                    .on_change(|state, action_tx| {
                        state.downcast_ref::<TableState>().and_then(|state| {
                            action_tx
                                .send(Action::Select {
                                    selected: state.selected(),
                                })
                                .ok()
                        });
                    })
                    .on_update(|state| {
                        let props = IssuesProps::from(state);

                        TableProps::default()
                            .columns(props.columns)
                            .items(state.issues())
                            .footer(!state.ui.show_search)
                            .page_size(state.ui.page_size)
                            .cutoff(props.cutoff, props.cutoff_after)
                            .to_boxed()
                    }),
            ),
            footer: Footer::new(state, action_tx).to_boxed(),
            on_update: None,
            on_change: None,
        }
    }

    fn on_update(mut self, callback: UpdateCallback<State>) -> Self {
        self.on_update = Some(callback);
        self
    }

    fn on_change(mut self, callback: EventCallback<Action>) -> Self {
        self.on_change = Some(callback);
        self
    }

    fn update(&mut self, state: &State) {
        // TODO call mapper here instead?
        self.props = IssuesProps::from(state);

        self.table.update(state);
        self.footer.update(state);
    }

    fn handle_key_event(&mut self, key: Key) {
        match key {
            Key::Char('\n') => {
                let operation = match self.props.mode {
                    Mode::Operation => Some(IssueOperation::Show.to_string()),
                    Mode::Id => None,
                };

                self.props
                    .selected
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
                self.props
                    .selected
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
                self.table.handle_key_event(key);
            }
        }
    }
}

impl<'a, B: Backend> Issues<'a, B> {
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
            .map(|selected| TableUtils::progress(selected, props.issues.len(), props.page_size))
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

impl<'a: 'static, B> Widget<B, State, Action> for Issues<'a, B>
where
    B: Backend + 'a,
{
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, props: Option<&dyn Any>) {
        let props = props
            .and_then(|props| props.downcast_ref::<IssuesProps>())
            .unwrap_or(&self.props);

        let header_height = 3_usize;

        let page_size = if props.show_search {
            self.table.render(frame, area, None);

            (area.height as usize).saturating_sub(header_height)
        } else {
            let layout = Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(area);

            self.table.render(frame, layout[0], None);
            self.footer.render(
                frame,
                layout[1],
                Some(&FooterProps::default().columns(Self::build_footer(props, props.selected))),
            );

            (area.height as usize).saturating_sub(header_height)
        };

        if page_size != props.page_size {
            let _ = self.action_tx.send(Action::PageSize(page_size));
        }
    }
}

pub struct Search<B: Backend> {
    /// Message sender
    action_tx: UnboundedSender<Action>,
    /// Custom update handler
    on_update: Option<UpdateCallback<State>>,
    /// Additional custom event handler
    on_change: Option<EventCallback<Action>>,
    /// Search input field
    input: BoxedWidget<B>,
}

impl<B: Backend> View<State, Action> for Search<B> {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let input = TextField::new(state, action_tx.clone())
            .on_change(|state, action_tx| {
                state.downcast_ref::<TextFieldState>().and_then(|state| {
                    action_tx
                        .send(Action::UpdateSearch {
                            value: state.text.clone(),
                        })
                        .ok()
                });
            })
            .on_update(|state| {
                TextFieldProps::default()
                    .text(&state.search.read().to_string())
                    .title("Search")
                    .inline(true)
                    .to_boxed()
            })
            .to_boxed();
        Self {
            action_tx,
            input,
            on_update: None,
            on_change: None,
        }
    }

    fn on_update(mut self, callback: UpdateCallback<State>) -> Self {
        self.on_update = Some(callback);
        self
    }

    fn on_change(mut self, callback: EventCallback<Action>) -> Self {
        self.on_change = Some(callback);
        self
    }

    fn update(&mut self, state: &State) {
        self.input.update(state);
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
                self.input.handle_key_event(key);
            }
        }
    }
}

impl<B> Widget<B, State, Action> for Search<B>
where
    B: Backend,
{
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, _props: Option<&dyn Any>) {
        let layout = Layout::horizontal(Constraint::from_mins([0]))
            .horizontal_margin(1)
            .split(area);

        self.input.render(frame, layout[0], None);
    }
}

#[derive(Clone)]
pub struct HelpProps<'a> {
    content: Text<'a>,
    focus: bool,
    page_size: usize,
    progress: usize,
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
            progress: state.help.progress,
        }
    }
}

pub struct Help<'a, B: Backend> {
    /// Internal properties
    props: HelpProps<'a>,
    /// Message sender
    action_tx: UnboundedSender<Action>,
    /// Custom update handler
    on_update: Option<UpdateCallback<State>>,
    /// Additional custom event handler
    on_change: Option<EventCallback<Action>>,
    /// Container header
    header: BoxedWidget<B>,
    /// Content widget
    content: BoxedWidget<B>,
    /// Container footer
    footer: BoxedWidget<B>,
}

impl<'a, B: Backend> View<State, Action> for Help<'a, B> {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
            props: HelpProps::from(state),
            header: Header::new(state, action_tx.clone()).to_boxed(),
            content: Paragraph::new(state, action_tx.clone())
                .on_update(|state| {
                    let props = HelpProps::from(state);

                    ParagraphProps::default()
                        .text(&props.content)
                        .page_size(props.page_size)
                        .focus(props.focus)
                        .to_boxed()
                })
                .on_change(|state, action_tx| {
                    state.downcast_ref::<ParagraphState>().and_then(|state| {
                        action_tx
                            .send(Action::ScrollHelp {
                                progress: state.progress,
                            })
                            .ok()
                    });
                })
                .to_boxed(),
            footer: Footer::new(state, action_tx).to_boxed(),
            on_update: None,
            on_change: None,
        }
    }

    fn on_update(mut self, callback: UpdateCallback<State>) -> Self {
        self.on_update = Some(callback);
        self
    }

    fn on_change(mut self, callback: EventCallback<Action>) -> Self {
        self.on_change = Some(callback);
        self
    }

    fn update(&mut self, state: &State) {
        self.props = HelpProps::from(state);

        self.header.update(state);
        self.content.update(state);
        self.footer.update(state);
    }

    fn handle_key_event(&mut self, key: termion::event::Key) {
        match key {
            Key::Esc => {
                let _ = self.action_tx.send(Action::Exit { selection: None });
            }
            Key::Char('?') => {
                let _ = self.action_tx.send(Action::CloseHelp);
            }
            _ => {
                self.content.handle_key_event(key);
            }
        }
    }
}

impl<'a: 'static, B> Widget<B, State, Action> for Help<'a, B>
where
    B: Backend,
{
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, props: Option<&dyn Any>) {
        let props = props
            .and_then(|props| props.downcast_ref::<HelpProps>())
            .unwrap_or(&self.props);

        let [header_area, content_area, footer_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .areas(area);
        let progress = span::default(format!("{}%", props.progress)).dim();

        self.header.render(
            frame,
            header_area,
            Some(
                &HeaderProps::default()
                    .columns([Column::new(" Help ", Constraint::Fill(1))].to_vec())
                    .focus(props.focus),
            ),
        );

        self.content.render(
            frame,
            content_area,
            Some(
                &ParagraphProps::default()
                    .text(&props.content)
                    .page_size(props.page_size)
                    .focus(props.focus),
            ),
        );

        self.footer.render(
            frame,
            footer_area,
            Some(
                &FooterProps::default()
                    .columns(
                        [
                            Column::new(Text::raw(""), Constraint::Fill(1)),
                            Column::new(progress, Constraint::Min(4)),
                        ]
                        .to_vec(),
                    )
                    .focus(props.focus),
            ),
        );

        let page_size = content_area.height as usize;
        if page_size != props.page_size {
            let _ = self.action_tx.send(Action::PageSize(page_size));
        }
    }
}
