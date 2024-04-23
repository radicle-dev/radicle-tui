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
use tui::ui::widget::container::{
    Container, ContainerProps, Footer, FooterProps, Header, HeaderProps,
};
use tui::ui::widget::input::{TextField, TextFieldProps, TextFieldState};
use tui::ui::widget::text::{Paragraph, ParagraphProps, ParagraphState};
use tui::ui::widget::{self, BaseView};
use tui::ui::widget::{
    Column, EventCallback, Properties, Shortcuts, ShortcutsProps, Table, TableProps, TableUtils,
    UpdateCallback, View, Widget,
};
use tui::Selection;

use crate::tui_issue::common::IssueOperation;
use crate::tui_issue::common::Mode;

use super::{Action, State};

type BoxedWidget<B> = widget::BoxedWidget<B, State, Action>;

#[derive(Clone)]
struct BrowsePageProps<'a> {
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
    shortcuts: Vec<(&'a str, &'a str)>,
}

impl<'a> From<&State> for BrowsePageProps<'a> {
    fn from(state: &State) -> Self {
        use radicle::issue::State;

        let issues = state.browser.issues();

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
            search: state.browser.search.read(),
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
            page_size: state.browser.page_size,
            show_search: state.browser.show_search,
            selected: state.browser.selected,
            shortcuts: match state.mode {
                Mode::Id => vec![("enter", "select"), ("/", "search")],
                Mode::Operation => vec![
                    ("enter", "show"),
                    ("e", "edit"),
                    ("/", "search"),
                    ("?", "help"),
                ],
            },
        }
    }
}

impl<'a> Properties for BrowsePageProps<'a> {}

pub struct BrowsePage<'a, B> {
    /// Internal base
    base: BaseView<State, Action, BrowsePageProps<'a>>,
    /// Patches widget
    issues: BoxedWidget<B>,
    /// Search widget
    search: BoxedWidget<B>,
    /// Shortcut widget
    shortcuts: BoxedWidget<B>,
}

impl<'a: 'static, B> View<State, Action> for BrowsePage<'a, B>
where
    B: Backend + 'a,
{
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self {
        let props = BrowsePageProps::from(state);

        Self {
            base: BaseView {
                action_tx: action_tx.clone(),
                props: BrowsePageProps::from(state),
                on_update: None,
                on_event: None,
            },
            issues: Container::new(state, action_tx.clone())
                .header(
                    Header::new(state, action_tx.clone())
                        .columns(props.columns.clone())
                        .cutoff(props.cutoff, props.cutoff_after)
                        .focus(props.focus)
                        .to_boxed(),
                )
                .content(Box::<Table<State, Action, IssueItem>>::new(
                    Table::new(state, action_tx.clone())
                        .on_event(|state, action_tx| {
                            state.downcast_ref::<TableState>().and_then(|state| {
                                action_tx
                                    .send(Action::Select {
                                        selected: state.selected(),
                                    })
                                    .ok()
                            });
                        })
                        .on_update(|state| {
                            let props = BrowsePageProps::from(state);

                            TableProps::default()
                                .columns(props.columns)
                                .items(state.browser.issues())
                                .footer(!state.browser.show_search)
                                .page_size(state.browser.page_size)
                                .cutoff(props.cutoff, props.cutoff_after)
                                .to_boxed()
                        }),
                ))
                .footer(
                    Footer::new(state, action_tx.clone())
                        .on_update(|state| {
                            let props = BrowsePageProps::from(state);

                            FooterProps::default()
                                .columns(Self::build_footer(&props, props.selected))
                                .to_boxed()
                        })
                        .to_boxed(),
                )
                .to_boxed(),
            search: Search::new(state, action_tx.clone()).to_boxed(),
            shortcuts: Shortcuts::new(state, action_tx.clone()).to_boxed(),
        }
    }

    fn on_update(mut self, callback: UpdateCallback<State>) -> Self {
        self.base.on_update = Some(callback);
        self
    }

    fn on_event(mut self, callback: EventCallback<Action>) -> Self {
        self.base.on_event = Some(callback);
        self
    }

    fn update(&mut self, state: &State) {
        self.base.props = BrowsePageProps::from_callback(self.base.on_update, state)
            .unwrap_or(BrowsePageProps::from(state));

        self.issues.update(state);
        self.search.update(state);
        self.shortcuts.update(state);
    }

    fn handle_key_event(&mut self, key: Key) {
        if self.base.props.show_search {
            self.search.handle_key_event(key);
        } else {
            match key {
                Key::Esc | Key::Ctrl('c') => {
                    let _ = self.base.action_tx.send(Action::Exit { selection: None });
                }
                Key::Char('?') => {
                    let _ = self.base.action_tx.send(Action::OpenHelp);
                }
                Key::Char('/') => {
                    let _ = self.base.action_tx.send(Action::OpenSearch);
                }
                Key::Char('\n') => {
                    let operation = match self.base.props.mode {
                        Mode::Operation => Some(IssueOperation::Show.to_string()),
                        Mode::Id => None,
                    };

                    self.base
                        .props
                        .selected
                        .and_then(|selected| self.base.props.issues.get(selected))
                        .and_then(|issue| {
                            self.base
                                .action_tx
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
                    self.base
                        .props
                        .selected
                        .and_then(|selected| self.base.props.issues.get(selected))
                        .and_then(|issue| {
                            self.base
                                .action_tx
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
                    self.issues.handle_key_event(key);
                }
            }
        }
    }
}

impl<'a, B: Backend> BrowsePage<'a, B> {
    fn build_footer(props: &BrowsePageProps<'a>, selected: Option<usize>) -> Vec<Column<'a>> {
        let search = Line::from(vec![
            span::default(" Search ").cyan().dim().reversed(),
            span::default(" "),
            span::default(&props.search).gray().dim(),
        ]);

        let open = Line::from(vec![
            span::positive(&props.stats.get("Open").unwrap_or(&0).to_string()).dim(),
            span::default(" Open").dim(),
        ]);
        let solved = Line::from(vec![
            span::default(&props.stats.get("Solved").unwrap_or(&0).to_string())
                .magenta()
                .dim(),
            span::default(" Solved").dim(),
        ]);
        let closed = Line::from(vec![
            span::default(&props.stats.get("Closed").unwrap_or(&0).to_string())
                .magenta()
                .dim(),
            span::default(" Closed").dim(),
        ]);
        let sum = Line::from(vec![
            span::default("Σ ").dim(),
            span::default(&props.issues.len().to_string()).dim(),
        ]);

        let progress = selected
            .map(|selected| TableUtils::progress(selected, props.issues.len(), props.page_size))
            .unwrap_or_default();
        let progress = span::default(&format!("{}%", progress)).dim();

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

impl<'a: 'static, B> Widget<B, State, Action> for BrowsePage<'a, B>
where
    B: Backend + 'a,
{
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, props: Option<Box<dyn Any>>) {
        let props = props
            .and_then(BrowsePageProps::from_boxed_any)
            .unwrap_or(self.base.props.clone());

        let page_size = area.height.saturating_sub(6) as usize;

        let [content_area, shortcuts_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);

        if props.show_search {
            let [table_area, search_area] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(content_area);

            self.issues.render(
                frame,
                table_area,
                Some(
                    ContainerProps::default()
                        .hide_footer(props.show_search)
                        .to_boxed(),
                ),
            );
            self.search.render(frame, search_area, None);
        } else {
            self.issues.render(
                frame,
                content_area,
                Some(
                    ContainerProps::default()
                        .hide_footer(props.show_search)
                        .to_boxed(),
                ),
            );
        }

        self.shortcuts.render(
            frame,
            shortcuts_area,
            Some(
                ShortcutsProps::default()
                    .shortcuts(&props.shortcuts)
                    .to_boxed(),
            ),
        );

        if page_size != props.page_size {
            let _ = self.base.action_tx.send(Action::BrowserPageSize(page_size));
        }
    }
}

pub struct SearchProps {}

impl Properties for SearchProps {}

pub struct Search<B: Backend> {
    /// Internal base
    base: BaseView<State, Action, SearchProps>,
    /// Search input field
    input: BoxedWidget<B>,
}

impl<B: Backend> View<State, Action> for Search<B> {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let input = TextField::new(state, action_tx.clone())
            .on_event(|state, action_tx| {
                state.downcast_ref::<TextFieldState>().and_then(|state| {
                    action_tx
                        .send(Action::UpdateSearch {
                            value: state.text.clone().unwrap_or_default(),
                        })
                        .ok()
                });
            })
            .on_update(|state| {
                TextFieldProps::default()
                    .text(&state.browser.search.read().to_string())
                    .title("Search")
                    .inline(true)
                    .to_boxed()
            })
            .to_boxed();
        Self {
            base: BaseView {
                action_tx: action_tx.clone(),
                props: SearchProps {},
                on_update: None,
                on_event: None,
            },
            input,
        }
    }

    fn on_update(mut self, callback: UpdateCallback<State>) -> Self {
        self.base.on_update = Some(callback);
        self
    }

    fn on_event(mut self, callback: EventCallback<Action>) -> Self {
        self.base.on_event = Some(callback);
        self
    }

    fn update(&mut self, state: &State) {
        self.input.update(state);
    }

    fn handle_key_event(&mut self, key: termion::event::Key) {
        match key {
            Key::Esc => {
                let _ = self.base.action_tx.send(Action::CloseSearch);
            }
            Key::Char('\n') => {
                let _ = self.base.action_tx.send(Action::ApplySearch);
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
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, _props: Option<Box<dyn Any>>) {
        let layout = Layout::horizontal(Constraint::from_mins([0]))
            .horizontal_margin(1)
            .split(area);

        self.input.render(frame, layout[0], None);
    }
}

#[derive(Clone)]
struct HelpPageProps<'a> {
    focus: bool,
    page_size: usize,
    help_progress: usize,
    shortcuts: Vec<(&'a str, &'a str)>,
}

impl<'a> From<&State> for HelpPageProps<'a> {
    fn from(state: &State) -> Self {
        Self {
            focus: false,
            page_size: state.help.page_size,
            help_progress: state.help.progress,
            shortcuts: vec![("?", "close")],
        }
    }
}

impl<'a> Properties for HelpPageProps<'a> {}

pub struct HelpPage<'a, B>
where
    B: Backend,
{
    /// Internal base
    base: BaseView<State, Action, HelpPageProps<'a>>,
    /// Content widget
    content: BoxedWidget<B>,
    /// Shortcut widget
    shortcuts: BoxedWidget<B>,
}

impl<'a: 'static, B> View<State, Action> for HelpPage<'a, B>
where
    B: Backend + 'a,
{
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            base: BaseView {
                action_tx: action_tx.clone(),
                props: HelpPageProps::from(state),
                on_update: None,
                on_event: None,
            },
            content: Container::new(state, action_tx.clone())
                .header(
                    Header::new(state, action_tx.clone())
                        .on_update(|state| {
                            let props = HelpPageProps::from(state);

                            HeaderProps::default()
                                .columns([Column::new(" Help ", Constraint::Fill(1))].to_vec())
                                .focus(props.focus)
                                .to_boxed()
                        })
                        .to_boxed(),
                )
                .content(
                    Paragraph::new(state, action_tx.clone())
                        .on_update(|state| {
                            let props = HelpPageProps::from(state);

                            ParagraphProps::default()
                                .text(&help_text())
                                .page_size(props.page_size)
                                .focus(props.focus)
                                .to_boxed()
                        })
                        .on_event(|state, action_tx| {
                            state.downcast_ref::<ParagraphState>().and_then(|state| {
                                action_tx
                                    .send(Action::ScrollHelp {
                                        progress: state.progress,
                                    })
                                    .ok()
                            });
                        })
                        .to_boxed(),
                )
                .footer(
                    Footer::new(state, action_tx.clone())
                        .on_update(|state| {
                            let props = HelpPageProps::from(state);

                            FooterProps::default()
                                .columns(
                                    [
                                        Column::new(Text::raw(""), Constraint::Fill(1)),
                                        Column::new(
                                            span::default(&format!("{}%", props.help_progress))
                                                .dim(),
                                            Constraint::Min(4),
                                        ),
                                    ]
                                    .to_vec(),
                                )
                                .focus(props.focus)
                                .to_boxed()
                        })
                        .to_boxed(),
                )
                .to_boxed(),
            shortcuts: Shortcuts::new(state, action_tx.clone()).to_boxed(),
        }
    }

    fn on_update(mut self, callback: UpdateCallback<State>) -> Self {
        self.base.on_update = Some(callback);
        self
    }

    fn on_event(mut self, callback: EventCallback<Action>) -> Self {
        self.base.on_event = Some(callback);
        self
    }

    fn update(&mut self, state: &State) {
        self.base.props = HelpPageProps::from_callback(self.base.on_update, state)
            .unwrap_or(HelpPageProps::from(state));

        self.content.update(state);
    }

    fn handle_key_event(&mut self, key: termion::event::Key) {
        match key {
            Key::Esc | Key::Ctrl('c') => {
                let _ = self.base.action_tx.send(Action::Exit { selection: None });
            }
            Key::Char('?') => {
                let _ = self.base.action_tx.send(Action::LeavePage);
            }
            _ => {
                self.content.handle_key_event(key);
            }
        }
    }
}

impl<'a: 'static, B> Widget<B, State, Action> for HelpPage<'a, B>
where
    B: Backend + 'a,
{
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, props: Option<Box<dyn Any>>) {
        let props = props
            .and_then(HelpPageProps::from_boxed_any)
            .unwrap_or(self.base.props.clone());

        let page_size = area.height.saturating_sub(6) as usize;

        let [content_area, shortcuts_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);

        self.content.render(frame, content_area, None);
        self.shortcuts.render(
            frame,
            shortcuts_area,
            Some(
                ShortcutsProps::default()
                    .shortcuts(&props.shortcuts)
                    .to_boxed(),
            ),
        );

        if page_size != props.page_size {
            let _ = self.base.action_tx.send(Action::HelpPageSize(page_size));
        }
    }
}

fn help_text() -> Text<'static> {
    Text::from(
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
            Line::raw(""),
            Line::raw(""),
        ]
        .to_vec())
}
