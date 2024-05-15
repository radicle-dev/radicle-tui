use std::collections::HashMap;
use std::str::FromStr;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::layout::{Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};

use radicle_tui as tui;

use tui::ui::items::{NotificationItem, NotificationItemFilter, NotificationState};
use tui::ui::span;
use tui::ui::widget;
use tui::ui::widget::container::{
    Column, Container, ContainerProps, Footer, FooterProps, Header, HeaderProps, SectionGroup,
    SectionGroupProps,
};
use tui::ui::widget::input::{TextField, TextFieldProps};
use tui::ui::widget::list::{Table, TableProps, TableUtils};
use tui::ui::widget::text::{Paragraph, ParagraphProps};
use tui::ui::widget::window::{Shortcuts, ShortcutsProps};
use tui::ui::widget::{BoxedAny, Properties, RenderProps, Widget, WidgetBase};

use tui::Selection;

use crate::tui_inbox::common::{InboxOperation, Mode, RepositoryMode, SelectionMode};

use super::{Action, State};

type BoxedWidget = widget::BoxedWidget<State, Action>;

#[derive(Clone)]
pub struct BrowserProps<'a> {
    /// Application mode: openation and id or id only.
    mode: Mode,
    /// Table title
    header: String,
    /// Filtered notifications.
    notifications: Vec<NotificationItem>,
    /// Current (selected) table index
    selected: Option<usize>,
    /// Notification statistics.
    stats: HashMap<String, usize>,
    /// Table columns
    columns: Vec<Column<'a>>,
    /// Max. width, before columns are cut-off.
    cutoff: usize,
    /// Column index that marks where to cut.
    cutoff_after: usize,
    /// Current page size (height of table content).
    page_size: usize,
    /// If search widget should be shown.
    show_search: bool,
    /// Current search string.
    search: String,
}

impl<'a> From<&State> for BrowserProps<'a> {
    fn from(state: &State) -> Self {
        let header = match state.mode.repository() {
            RepositoryMode::Contextual => state.project.name().to_string(),
            RepositoryMode::All => "All repositories".to_string(),
            RepositoryMode::ByRepo((_, name)) => name.clone().unwrap_or_default(),
        };

        let notifications = state.browser.notifications();

        // Compute statistics
        let mut seen = 0;
        let mut unseen = 0;
        for notification in &notifications {
            if notification.seen {
                seen += 1;
            } else {
                unseen += 1;
            }
        }
        let stats = HashMap::from([("Seen".to_string(), seen), ("Unseen".to_string(), unseen)]);

        Self {
            mode: state.mode.clone(),
            header,
            notifications,
            selected: state.browser.selected,
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
            page_size: state.browser.page_size,
            search: state.browser.search.read(),
            show_search: state.browser.show_search,
        }
    }
}

impl<'a> Properties for BrowserProps<'a> {}
impl<'a> BoxedAny for BrowserProps<'a> {}

pub struct Browser<'a> {
    /// Internal base
    base: WidgetBase<State, Action>,
    /// Internal props
    props: BrowserProps<'a>,
    /// Notification widget
    notifications: BoxedWidget,
    /// Search widget
    search: BoxedWidget,
}

impl<'a: 'static> Widget for Browser<'a> {
    type Action = Action;
    type State = State;

    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self {
        let props = BrowserProps::from(state);

        Self {
            base: WidgetBase::new(action_tx.clone()),
            props: props.clone(),
            notifications: Container::new(state, action_tx.clone())
                .header(
                    Header::new(state, action_tx.clone())
                        .columns(
                            [
                                Column::new("", Constraint::Length(0)),
                                Column::new(Text::from(props.header), Constraint::Fill(1)),
                            ]
                            .to_vec(),
                        )
                        .cutoff(props.cutoff, props.cutoff_after)
                        .to_boxed(),
                )
                .content(Box::<Table<State, Action, NotificationItem, 9>>::new(
                    Table::new(state, action_tx.clone())
                        .on_event(|table| {
                            table
                                .downcast_mut::<Table<State, Action, NotificationItem, 9>>()
                                .and_then(|table| {
                                    table
                                        .send(Action::Select {
                                            selected: table.selected(),
                                        })
                                        .ok()
                                });
                        })
                        .on_update(|state| {
                            let props = BrowserProps::from(state);

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
                            FooterProps::default()
                                .columns(browse_footer(&BrowserProps::from(state)))
                                .to_boxed()
                        })
                        .to_boxed(),
                )
                .on_update(|state| {
                    ContainerProps::default()
                        .hide_footer(BrowserProps::from(state).show_search)
                        .to_boxed()
                })
                .to_boxed(),
            search: Search::new(state, action_tx.clone()).to_boxed(),
        }
    }

    fn handle_event(&mut self, key: Key) {
        if self.props.show_search {
            self.search.handle_event(key);
        } else {
            match key {
                Key::Char('/') => {
                    let _ = self.send(Action::OpenSearch);
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

                            self.base
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
                            self.base
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
                _ => self.notifications.handle_event(key),
            }
        }
    }

    fn update(&mut self, state: &State) {
        self.props = BrowserProps::from_callback(self.base.on_update, state)
            .unwrap_or(BrowserProps::from(state));

        self.notifications.update(state);
        self.search.update(state);
    }

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        if self.props.show_search {
            let [table_area, search_area] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(props.area);

            self.notifications
                .render(frame, RenderProps::from(table_area));
            self.search
                .render(frame, RenderProps::from(search_area).focus(props.focus));
        } else {
            self.notifications.render(frame, props);
        }
    }

    fn base(&self) -> &WidgetBase<State, Action> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut WidgetBase<State, Action> {
        &mut self.base
    }
}

#[derive(Clone)]
struct BrowserPageProps<'a> {
    /// Current page size (height of table content).
    page_size: usize,
    /// If this pages' keys should be handled (`false` if search is shown).
    handle_keys: bool,
    /// This pages' shortcuts.
    shortcuts: Vec<(&'a str, &'a str)>,
}

impl<'a> From<&State> for BrowserPageProps<'a> {
    fn from(state: &State) -> Self {
        Self {
            page_size: state.browser.page_size,
            handle_keys: !state.browser.show_search,
            shortcuts: if state.browser.show_search {
                vec![("esc", "cancel"), ("enter", "apply")]
            } else {
                match state.mode.selection() {
                    SelectionMode::Id => vec![("enter", "select"), ("/", "search")],
                    SelectionMode::Operation => vec![
                        ("enter", "show"),
                        ("c", "clear"),
                        ("/", "search"),
                        ("?", "help"),
                    ],
                }
            },
        }
    }
}

impl<'a> Properties for BrowserPageProps<'a> {}
impl<'a> BoxedAny for BrowserPageProps<'a> {}

pub struct BrowserPage<'a> {
    /// Internal base
    base: WidgetBase<State, Action>,
    /// Internal props
    props: BrowserPageProps<'a>,
    /// Sections widget
    sections: BoxedWidget,
    /// Shortcut widget
    shortcuts: BoxedWidget,
}

impl<'a: 'static> Widget for BrowserPage<'a> {
    type Action = Action;
    type State = State;

    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self {
        let props = BrowserPageProps::from(state);

        Self {
            base: WidgetBase::new(action_tx.clone()),
            props: props.clone(),
            sections: SectionGroup::new(state, action_tx.clone())
                .section(Browser::new(state, action_tx.clone()).to_boxed())
                .on_update(|state| {
                    let props = BrowserPageProps::from(state);
                    SectionGroupProps::default()
                        .handle_keys(props.handle_keys)
                        .to_boxed()
                })
                .to_boxed(),
            shortcuts: Shortcuts::new(state, action_tx.clone())
                .on_update(|state| {
                    ShortcutsProps::default()
                        .shortcuts(&BrowserPageProps::from(state).shortcuts)
                        .to_boxed()
                })
                .to_boxed(),
        }
    }

    fn handle_event(&mut self, key: Key) {
        self.sections.handle_event(key);

        if self.props.handle_keys {
            match key {
                Key::Esc | Key::Ctrl('c') => {
                    let _ = self.send(Action::Exit { selection: None });
                }
                Key::Char('?') => {
                    let _ = self.send(Action::OpenHelp);
                }
                _ => {}
            }
        }
    }

    fn update(&mut self, state: &State) {
        self.props = BrowserPageProps::from_callback(self.base.on_update, state)
            .unwrap_or(BrowserPageProps::from(state));

        self.sections.update(state);
        self.shortcuts.update(state);
    }

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let page_size = props.area.height.saturating_sub(6) as usize;

        let [content_area, shortcuts_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(props.area);

        self.sections.render(
            frame,
            RenderProps::from(content_area)
                .layout(Layout::horizontal([Constraint::Min(1)]))
                .focus(true),
        );
        self.shortcuts
            .render(frame, RenderProps::from(shortcuts_area));

        if page_size != self.props.page_size {
            let _ = self.send(Action::BrowserPageSize(page_size));
        }
    }

    fn base(&self) -> &WidgetBase<State, Action> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut WidgetBase<State, Action> {
        &mut self.base
    }
}

pub struct SearchProps {}

impl Properties for SearchProps {}

pub struct Search {
    /// Internal base
    base: WidgetBase<State, Action>,
    /// Internal props
    _props: SearchProps,
    /// Search input field
    input: BoxedWidget,
}

impl Widget for Search {
    type Action = Action;
    type State = State;

    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            base: WidgetBase::new(action_tx.clone()),
            _props: SearchProps {},
            input: TextField::new(state, action_tx.clone())
                .on_event(|widget| {
                    widget
                        .downcast_mut::<TextField<State, Action>>()
                        .and_then(|field| {
                            field
                                .send(Action::UpdateSearch {
                                    value: field.text().unwrap_or(&String::new()).to_string(),
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
                .to_boxed(),
        }
    }

    fn handle_event(&mut self, key: termion::event::Key) {
        match key {
            Key::Esc => {
                let _ = self.send(Action::CloseSearch);
            }
            Key::Char('\n') => {
                let _ = self.send(Action::ApplySearch);
            }
            _ => {
                self.input.handle_event(key);
            }
        }
    }

    fn update(&mut self, state: &State) {
        self.input.update(state);
    }

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let layout = Layout::horizontal(Constraint::from_mins([0]))
            .horizontal_margin(1)
            .split(props.area);

        self.input.render(frame, RenderProps::from(layout[0]));
    }

    fn base(&self) -> &WidgetBase<State, Action> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut WidgetBase<State, Action> {
        &mut self.base
    }
}

#[derive(Clone)]
struct HelpPageProps<'a> {
    /// Current page size (height of table content).
    page_size: usize,
    /// Scroll progress of help paragraph.
    help_progress: usize,
    /// This pages' shortcuts.
    shortcuts: Vec<(&'a str, &'a str)>,
}

impl<'a> From<&State> for HelpPageProps<'a> {
    fn from(state: &State) -> Self {
        Self {
            page_size: state.help.page_size,
            help_progress: state.help.progress,
            shortcuts: vec![("?", "close")],
        }
    }
}

impl<'a> Properties for HelpPageProps<'a> {}
impl<'a> BoxedAny for HelpPageProps<'a> {}

pub struct HelpPage<'a> {
    /// Internal base
    base: WidgetBase<State, Action>,
    /// Internal props
    props: HelpPageProps<'a>,
    /// Content widget
    content: BoxedWidget,
    /// Shortcut widget
    shortcuts: BoxedWidget,
}

impl<'a: 'static> Widget for HelpPage<'a> {
    type Action = Action;
    type State = State;

    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            base: WidgetBase::new(action_tx.clone()),
            props: HelpPageProps::from(state),
            content: Container::new(state, action_tx.clone())
                .header(
                    Header::new(state, action_tx.clone())
                        .on_update(|_| {
                            HeaderProps::default()
                                .columns([Column::new(" Help ", Constraint::Fill(1))].to_vec())
                                .to_boxed()
                        })
                        .to_boxed(),
                )
                .content(
                    Paragraph::new(state, action_tx.clone())
                        .on_event(|paragraph| {
                            paragraph
                                .downcast_mut::<Paragraph<'_, State, Action>>()
                                .and_then(|paragraph| {
                                    paragraph
                                        .send(Action::ScrollHelp {
                                            progress: paragraph.progress(),
                                        })
                                        .ok()
                                });
                        })
                        .on_update(|state| {
                            let props = HelpPageProps::from(state);

                            ParagraphProps::default()
                                .text(&help_text())
                                .page_size(props.page_size)
                                .to_boxed()
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
                                .to_boxed()
                        })
                        .to_boxed(),
                )
                .to_boxed(),
            shortcuts: Shortcuts::new(state, action_tx.clone())
                .on_update(|state| {
                    ShortcutsProps::default()
                        .shortcuts(&HelpPageProps::from(state).shortcuts)
                        .to_boxed()
                })
                .to_boxed(),
        }
    }

    fn handle_event(&mut self, key: termion::event::Key) {
        match key {
            Key::Esc | Key::Ctrl('c') => {
                let _ = self.send(Action::Exit { selection: None });
            }
            Key::Char('?') => {
                let _ = self.send(Action::LeavePage);
            }
            _ => {
                self.content.handle_event(key);
            }
        }
    }

    fn update(&mut self, state: &State) {
        self.props = HelpPageProps::from_callback(self.base.on_update, state)
            .unwrap_or(HelpPageProps::from(state));

        self.content.update(state);
        self.shortcuts.update(state);
    }

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let page_size = props.area.height.saturating_sub(6) as usize;

        let [content_area, shortcuts_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(props.area);

        self.content
            .render(frame, RenderProps::from(content_area).focus(true));
        self.shortcuts
            .render(frame, RenderProps::from(shortcuts_area));

        if page_size != self.props.page_size {
            let _ = self.send(Action::HelpPageSize(page_size));
        }
    }

    fn base(&self) -> &WidgetBase<State, Action> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut WidgetBase<State, Action> {
        &mut self.base
    }
}

fn browse_footer<'a>(props: &BrowserProps<'a>) -> Vec<Column<'a>> {
    let search = Line::from(vec![
        span::default(" Search ").cyan().dim().reversed(),
        span::default(" "),
        span::default(&props.search.to_string()).gray().dim(),
    ]);

    let seen = Line::from(vec![
        span::positive(&props.stats.get("Seen").unwrap_or(&0).to_string()).dim(),
        span::default(" Seen").dim(),
    ]);
    let unseen = Line::from(vec![
        span::positive(&props.stats.get("Unseen").unwrap_or(&0).to_string())
            .magenta()
            .dim(),
        span::default(" Unseen").dim(),
    ]);

    let progress = props
        .selected
        .map(|selected| TableUtils::progress(selected, props.notifications.len(), props.page_size))
        .unwrap_or_default();
    let progress = span::default(&format!("{}%", progress)).dim();

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

fn help_text() -> Text<'static> {
    Text::from(
        [
            Line::from(Span::raw("Generic keybindings").cyan()),
            Line::raw(""),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "↑,k")).gray(),
                Span::raw(" "),
                Span::raw("move cursor one line up").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "↓,j")).gray(),
                Span::raw(" "),
                Span::raw("move cursor one line down").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "PageUp")).gray(),
                Span::raw(" "),
                Span::raw("move cursor one page up").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "PageDown")).gray(),
                Span::raw(" "),
                Span::raw("move cursor one page down").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "Home")).gray(),
                Span::raw(" "),
                Span::raw("move cursor to the first line").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "End")).gray(),
                Span::raw(" "),
                Span::raw("move cursor to the last line").gray().dim(),
            ]),
            Line::raw(""),
            Line::from(Span::raw("Specific keybindings").cyan()),
            Line::raw(""),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "enter")).gray(),
                Span::raw(" "),
                Span::raw("Select notification (if --mode id)").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "enter")).gray(),
                Span::raw(" "),
                Span::raw("Show notification").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "c")).gray(),
                Span::raw(" "),
                Span::raw("Clear notifications").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "/")).gray(),
                Span::raw(" "),
                Span::raw("Search").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "?")).gray(),
                Span::raw(" "),
                Span::raw("Show help").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "Esc")).gray(),
                Span::raw(" "),
                Span::raw("Quit / cancel").gray().dim(),
            ]),
            Line::raw(""),
            Line::from(Span::raw("Searching").cyan()),
            Line::raw(""),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "Pattern")).gray(),
                Span::raw(" "),
                Span::raw("is:<state> | is:patch | is:issue | <search>")
                    .gray()
                    .dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "Example")).gray(),
                Span::raw(" "),
                Span::raw("is:unseen is:patch Print").gray().dim(),
            ]),
            Line::raw(""),
            Line::raw(""),
        ]
        .to_vec(),
    )
}
