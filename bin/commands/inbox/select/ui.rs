use std::any::Any;
use std::collections::HashMap;
use std::str::FromStr;

use ratatui::widgets::TableState;
use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};

use radicle_tui as tui;

use tui::ui::items::{NotificationItem, NotificationItemFilter, NotificationState};
use tui::ui::span;
use tui::ui::widget::container::{
    Container, ContainerProps, Footer, FooterProps, Header, HeaderProps,
};
use tui::ui::widget::input::{TextField, TextFieldProps, TextFieldState};
use tui::ui::widget::text::{Paragraph, ParagraphProps, ParagraphState};
use tui::ui::widget::{self, TableUtils};
use tui::ui::widget::{
    Column, EventCallback, Properties, Shortcuts, ShortcutsProps, Table, TableProps,
    UpdateCallback, View, Widget,
};
use tui::Selection;

use crate::tui_inbox::common::{InboxOperation, Mode, RepositoryMode, SelectionMode};

use super::{Action, Page, State};

type BoxedWidget<B> = widget::BoxedWidget<B, State, Action>;

#[derive(Clone)]
pub struct WindowProps {
    page: Page,
}

impl From<&State> for WindowProps {
    fn from(state: &State) -> Self {
        Self {
            page: state.pages.peek().unwrap_or(&Page::Browse).clone(),
        }
    }
}

impl Properties for WindowProps {}

pub struct Window<B: Backend> {
    /// Internal properties
    props: WindowProps,
    /// Message sender
    _action_tx: UnboundedSender<Action>,
    /// Custom update handler
    on_update: Option<UpdateCallback<State>>,
    /// Additional custom event handler
    on_change: Option<EventCallback<Action>>,
    /// All pages known
    pages: HashMap<Page, BoxedWidget<B>>,
}

impl<'a: 'static, B> View<State, Action> for Window<B>
where
    B: Backend + 'a,
{
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            _action_tx: action_tx.clone(),
            props: WindowProps::from(state),
            pages: HashMap::from([
                (
                    Page::Browse,
                    BrowsePage::new(state, action_tx.clone()).to_boxed() as BoxedWidget<B>,
                ),
                (
                    Page::Help,
                    HelpPage::new(state, action_tx.clone()).to_boxed() as BoxedWidget<B>,
                ),
            ]),
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
        self.props = WindowProps::from(state);

        if let Some(page) = self.pages.get_mut(&self.props.page) {
            page.update(state);
        }
    }

    fn handle_key_event(&mut self, key: termion::event::Key) {
        if let Some(page) = self.pages.get_mut(&self.props.page) {
            page.handle_key_event(key);
        }
    }
}

impl<'a: 'static, B> Widget<B, State, Action> for Window<B>
where
    B: Backend + 'a,
{
    fn render(&self, frame: &mut ratatui::Frame, _area: Rect, props: Option<Box<dyn Any>>) {
        let props = props
            .and_then(WindowProps::from_boxed_any)
            .unwrap_or(self.props.clone());

        let area = frame.size();

        if let Some(page) = self.pages.get(&props.page) {
            page.render(frame, area, None);
        }
    }
}

#[derive(Clone)]
struct BrowsePageProps<'a> {
    notifications: Vec<NotificationItem>,
    selected: Option<usize>,
    mode: Mode,
    stats: HashMap<String, usize>,
    columns: Vec<Column<'a>>,
    cutoff: usize,
    cutoff_after: usize,
    focus: bool,
    page_size: usize,
    search: String,
    show_search: bool,
    shortcuts: Vec<(&'a str, &'a str)>,
}

impl<'a> From<&State> for BrowsePageProps<'a> {
    fn from(state: &State) -> Self {
        let mut seen = 0;
        let mut unseen = 0;

        let notifications = state.browser.notifications();

        // Compute statistics
        for notification in &notifications {
            if notification.seen {
                seen += 1;
            } else {
                unseen += 1;
            }
        }

        let stats = HashMap::from([("Seen".to_string(), seen), ("Unseen".to_string(), unseen)]);

        Self {
            notifications,
            mode: state.mode.clone(),
            stats,
            columns: [
                Column::new("", Constraint::Length(5)),
                Column::new("", Constraint::Length(3)),
                Column::new("", Constraint::Length(15))
                    .skip(*state.mode.repository() != RepositoryMode::All),
                Column::new("", Constraint::Length(25)),
                Column::new("", Constraint::Fill(1)),
                Column::new("", Constraint::Length(8)),
                Column::new("", Constraint::Length(10)),
                Column::new("", Constraint::Length(15)),
                Column::new("", Constraint::Length(18)),
            ]
            .to_vec(),
            cutoff: 200,
            cutoff_after: 5,
            focus: false,
            search: state.browser.search.read(),
            page_size: state.browser.page_size,
            show_search: state.browser.show_search,
            selected: state.browser.selected,
            shortcuts: match state.mode.selection() {
                SelectionMode::Id => vec![("enter", "select"), ("/", "search")],
                SelectionMode::Operation => vec![
                    ("enter", "show"),
                    ("c", "clear"),
                    ("/", "search"),
                    ("?", "help"),
                ],
            },
        }
    }
}

impl<'a> Properties for BrowsePageProps<'a> {}

struct BrowsePage<'a, B> {
    /// Internal properties
    props: BrowsePageProps<'a>,
    /// Message sender
    action_tx: UnboundedSender<Action>,
    /// Custom update handler
    on_update: Option<UpdateCallback<State>>,
    /// Additional custom event handler
    on_change: Option<EventCallback<Action>>,
    /// Notifications widget
    notifications: BoxedWidget<B>,
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
        let name = match state.mode.repository() {
            RepositoryMode::Contextual => state.project.name().to_string(),
            RepositoryMode::All => "All repositories".to_string(),
            RepositoryMode::ByRepo((_, name)) => name.clone().unwrap_or_default(),
        };

        Self {
            action_tx: action_tx.clone(),
            props: BrowsePageProps::from(state),
            notifications: Container::new(state, action_tx.clone())
                .header(
                    Header::new(state, action_tx.clone())
                        .columns(
                            [
                                Column::new("", Constraint::Length(0)),
                                Column::new(Text::from(name), Constraint::Fill(1)),
                            ]
                            .to_vec(),
                        )
                        .cutoff(props.cutoff, props.cutoff_after)
                        .focus(props.focus)
                        .to_boxed(),
                )
                .content(Box::<Table<State, Action, NotificationItem>>::new(
                    Table::new(state, action_tx.clone())
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
                            let props = BrowsePageProps::from(state);

                            TableProps::default()
                                .columns(props.columns)
                                .items(state.browser.notifications())
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
        // TODO call mapper here instead?
        self.props = BrowsePageProps::from(state);

        self.notifications.update(state);
        self.search.update(state);
        self.shortcuts.update(state);
    }

    fn handle_key_event(&mut self, key: Key) {
        if self.props.show_search {
            self.search.handle_key_event(key);
        } else {
            match key {
                Key::Esc | Key::Ctrl('c') => {
                    let _ = self.action_tx.send(Action::Exit { selection: None });
                }
                Key::Char('?') => {
                    let _ = self.action_tx.send(Action::OpenHelp);
                }
                Key::Char('/') => {
                    let _ = self.action_tx.send(Action::OpenSearch);
                }
                Key::Char('\n') => {
                    self.props
                        .selected
                        .and_then(|selected| self.props.notifications.get(selected))
                        .and_then(|notif| {
                            let selection = match self.props.mode.selection() {
                                SelectionMode::Operation => Selection::default()
                                    .with_operation(InboxOperation::Show.to_string())
                                    .with_id(notif.id),
                                SelectionMode::Id => Selection::default().with_id(notif.id),
                            };

                            self.action_tx
                                .send(Action::Exit {
                                    selection: Some(selection),
                                })
                                .ok()
                        });
                }
                Key::Char('c') => {
                    self.props
                        .selected
                        .and_then(|selected| self.props.notifications.get(selected))
                        .and_then(|notif| {
                            self.action_tx
                                .send(Action::Exit {
                                    selection: Some(
                                        Selection::default()
                                            .with_operation(InboxOperation::Clear.to_string())
                                            .with_id(notif.id),
                                    ),
                                })
                                .ok()
                        });
                }
                _ => {
                    self.notifications.handle_key_event(key);
                }
            }
        }
    }
}

impl<'a, B: Backend> BrowsePage<'a, B> {
    fn build_footer(props: &BrowsePageProps<'a>, selected: Option<usize>) -> Vec<Column<'a>> {
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

        let seen = Line::from(
            [
                span::positive(props.stats.get("Seen").unwrap_or(&0).to_string()).dim(),
                span::default(" Seen".to_string()).dim(),
            ]
            .to_vec(),
        );
        let unseen = Line::from(
            [
                span::positive(props.stats.get("Unseen").unwrap_or(&0).to_string())
                    .magenta()
                    .dim(),
                span::default(" Unseen".to_string()).dim(),
            ]
            .to_vec(),
        );

        let progress = selected
            .map(|selected| {
                TableUtils::progress(selected, props.notifications.len(), props.page_size)
            })
            .unwrap_or_default();
        let progress = span::default(format!("{}%", progress)).dim();

        match NotificationItemFilter::from_str(&props.search)
            .unwrap_or_default()
            .state()
        {
            Some(state) => {
                let block = match state {
                    NotificationState::Seen => seen,
                    NotificationState::Unseen => unseen,
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
                    Text::from(seen.clone()),
                    Constraint::Min(seen.width() as u16),
                ),
                Column::new(
                    Text::from(unseen.clone()),
                    Constraint::Min(unseen.width() as u16),
                ),
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
            .unwrap_or(self.props.clone());

        let page_size = area.height.saturating_sub(6) as usize;

        let [content_area, shortcuts_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);

        if props.show_search {
            let [table_area, search_area] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(content_area);

            self.notifications.render(
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
            self.notifications.render(
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
            let _ = self.action_tx.send(Action::BrowserPageSize(page_size));
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
    /// Internal properties
    props: HelpPageProps<'a>,
    /// Message sender
    action_tx: UnboundedSender<Action>,
    /// Custom update handler
    on_update: Option<UpdateCallback<State>>,
    /// Additional custom event handler
    on_change: Option<EventCallback<Action>>,
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
            action_tx: action_tx.clone(),
            props: HelpPageProps::from(state),
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
                                            span::default(format!("{}%", props.help_progress))
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
        self.props = HelpPageProps::from(state);

        self.content.update(state);
    }

    fn handle_key_event(&mut self, key: termion::event::Key) {
        match key {
            Key::Esc | Key::Ctrl('c') => {
                let _ = self.action_tx.send(Action::Exit { selection: None });
            }
            Key::Char('?') => {
                let _ = self.action_tx.send(Action::LeavePage);
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
            .unwrap_or(self.props.clone());

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
            let _ = self.action_tx.send(Action::HelpPageSize(page_size));
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
                    Span::raw("Select notification (if --mode id)").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "enter")).gray(),
                    Span::raw(" "),
                    Span::raw("Show notification").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "c")).gray(),
                    Span::raw(" "),
                    Span::raw("Clear notifications").gray().dim(),
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
                    Span::raw("is:<state> | is:patch | is:issue | <search>")
                        .gray()
                        .dim(),
                ]
                .to_vec(),
            ),
            Line::from(
                [
                    Span::raw(format!("{key:>10}", key = "Example")).gray(),
                    Span::raw(" "),
                    Span::raw("is:unseen is:patch Print").gray().dim(),
                ]
                .to_vec(),
            ),
            Line::raw(""),
            Line::raw(""),
        ]
        .to_vec(),
    )
}
