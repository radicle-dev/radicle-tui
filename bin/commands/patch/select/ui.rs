use std::collections::HashMap;
use std::str::FromStr;
use std::vec;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::layout::{Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};

use radicle::patch::{self, Status};

use radicle_tui as tui;

use tui::ui::items::{PatchItem, PatchItemFilter};
use tui::ui::span;
use tui::ui::widget::container::{
    Column, Container, ContainerProps, Footer, FooterProps, Header, HeaderProps, SectionGroup,
    SectionGroupProps,
};
use tui::ui::widget::input::{TextField, TextFieldProps};
use tui::ui::widget::list::{Table, TableProps, TableUtils};
use tui::ui::widget::text::{Paragraph, ParagraphProps};
use tui::ui::widget::window::{Shortcuts, ShortcutsProps};
use tui::ui::widget::{self, ViewProps};
use tui::ui::widget::{RenderProps, ToWidget, View};

use tui::{BoxedAny, Selection};

use crate::tui_patch::common::Mode;
use crate::tui_patch::common::PatchOperation;

use super::{Message, State};

type Widget = widget::Widget<State, Message>;

#[derive(Clone, Default)]
pub struct BrowserProps<'a> {
    /// Application mode: openation and id or id only.
    mode: Mode,
    /// Filtered patches.
    patches: Vec<PatchItem>,
    /// Current (selected) table index
    selected: Option<usize>,
    /// Patch statistics.
    stats: HashMap<String, usize>,
    /// Header columns
    header: Vec<Column<'a>>,
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
        let mut draft = 0;
        let mut open = 0;
        let mut archived = 0;
        let mut merged = 0;

        let patches = state.browser.patches();

        for patch in &patches {
            match patch.state {
                patch::State::Draft => draft += 1,
                patch::State::Open { conflicts: _ } => open += 1,
                patch::State::Archived => archived += 1,
                patch::State::Merged {
                    commit: _,
                    revision: _,
                } => merged += 1,
            }
        }

        let stats = HashMap::from([
            ("Draft".to_string(), draft),
            ("Open".to_string(), open),
            ("Archived".to_string(), archived),
            ("Merged".to_string(), merged),
        ]);

        Self {
            mode: state.mode.clone(),
            patches,
            selected: state.browser.selected,
            stats,
            header: [
                Column::new(" ● ", Constraint::Length(3)),
                Column::new("ID", Constraint::Length(8)),
                Column::new("Title", Constraint::Fill(1)),
                Column::new("Author", Constraint::Length(16)),
                Column::new("", Constraint::Length(16)),
                Column::new("Head", Constraint::Length(8)),
                Column::new("+", Constraint::Length(6)),
                Column::new("-", Constraint::Length(6)),
                Column::new("Updated", Constraint::Length(16)),
            ]
            .to_vec(),
            columns: [
                Column::new(" ● ", Constraint::Length(3)),
                Column::new("ID", Constraint::Length(8)),
                Column::new("Title", Constraint::Fill(1)),
                Column::new("Author", Constraint::Length(16)),
                Column::new("", Constraint::Length(16)),
                Column::new("Head", Constraint::Length(8)),
                Column::new("+", Constraint::Length(6)),
                Column::new("-", Constraint::Length(6)),
                Column::new("Updated", Constraint::Length(16)),
            ]
            .to_vec(),
            cutoff: 150,
            cutoff_after: 5,
            page_size: state.browser.page_size,
            show_search: state.browser.show_search,
            search: state.browser.search.read(),
        }
    }
}

pub struct Browser<'a> {
    /// Internal props
    props: BrowserProps<'a>,
    /// Patches widget
    patches: Widget,
    /// Search widget
    search: Widget,
}

impl<'a: 'static> Browser<'a> {
    fn new(tx: UnboundedSender<Message>) -> Self {
        let props = BrowserProps::default();
        Self {
            props: props.clone(),
            patches: Container::default()
                .header(
                    Header::default()
                        .columns(props.header.clone())
                        .cutoff(props.cutoff, props.cutoff_after)
                        .to_widget(tx.clone()),
                )
                .content(
                    Table::<State, Message, PatchItem, 9>::default()
                        .to_widget(tx.clone())
                        .on_event(|s, _| {
                            Some(Message::Select {
                                selected: s.and_then(|s| s.unwrap_usize()),
                            })
                        })
                        .on_update(|state| {
                            let props = BrowserProps::from(state);

                            TableProps::default()
                                .columns(props.columns)
                                .items(state.browser.patches())
                                .selected(state.browser.selected)
                                .footer(!state.browser.show_search)
                                .page_size(state.browser.page_size)
                                .cutoff(props.cutoff, props.cutoff_after)
                                .to_boxed_any()
                                .into()
                        }),
                )
                .footer(Footer::default().to_widget(tx.clone()).on_update(|state| {
                    let props = BrowserProps::from(state);

                    FooterProps::default()
                        .columns(browse_footer(&props, props.selected))
                        .to_boxed_any()
                        .into()
                }))
                .to_widget(tx.clone())
                .on_update(|state| {
                    ContainerProps::default()
                        .hide_footer(BrowserProps::from(state).show_search)
                        .to_boxed_any()
                        .into()
                }),
            search: Search::new(tx.clone()).to_widget(tx.clone()),
        }
    }
}

impl<'a: 'static> View for Browser<'a> {
    type Message = Message;
    type State = State;

    fn handle_event(&mut self, key: Key) -> Option<Self::Message> {
        if self.props.show_search {
            self.search.handle_event(key);
            None
        } else {
            match key {
                Key::Esc | Key::Ctrl('c') => Some(Message::Exit { selection: None }),
                Key::Char('?') => Some(Message::OpenHelp),
                Key::Char('/') => Some(Message::OpenSearch),
                Key::Char('\n') => {
                    let operation = match self.props.mode {
                        Mode::Operation => Some(PatchOperation::Show.to_string()),
                        Mode::Id => None,
                    };

                    self.props
                        .selected
                        .and_then(|selected| self.props.patches.get(selected))
                        .map(|patch| Message::Exit {
                            selection: Some(Selection {
                                operation,
                                ids: vec![patch.id],
                                args: vec![],
                            }),
                        })
                }
                Key::Char('c') => self
                    .props
                    .selected
                    .and_then(|selected| self.props.patches.get(selected))
                    .map(|patch| Message::Exit {
                        selection: Some(Selection {
                            operation: Some(PatchOperation::Checkout.to_string()),
                            ids: vec![patch.id],
                            args: vec![],
                        }),
                    }),
                Key::Char('d') => self
                    .props
                    .selected
                    .and_then(|selected| self.props.patches.get(selected))
                    .map(|patch| Message::Exit {
                        selection: Some(Selection {
                            operation: Some(PatchOperation::Diff.to_string()),
                            ids: vec![patch.id],
                            args: vec![],
                        }),
                    }),
                _ => {
                    self.patches.handle_event(key);
                    None
                }
            }
        }
    }

    fn update(&mut self, state: &Self::State, props: Option<ViewProps>) {
        if let Some(props) = props.and_then(|props| props.inner::<BrowserProps>()) {
            self.props = props;
        } else {
            self.props = BrowserProps::from(state);
        }

        self.patches.update(state);
        self.search.update(state);
    }

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        if self.props.show_search {
            let [table_area, search_area] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(props.area);

            self.patches.render(frame, RenderProps::from(table_area));
            self.search
                .render(frame, RenderProps::from(search_area).focus(props.focus));
        } else {
            self.patches.render(frame, props);
        }
    }
}

#[derive(Clone, Default)]
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
                match state.mode {
                    Mode::Id => vec![("enter", "select"), ("/", "search")],
                    Mode::Operation => vec![
                        ("enter", "show"),
                        ("c", "checkout"),
                        ("d", "diff"),
                        ("/", "search"),
                        ("?", "help"),
                    ],
                }
            },
        }
    }
}

pub struct BrowserPage<'a> {
    /// Internal props
    props: BrowserPageProps<'a>,
    /// Sections widget
    sections: Widget,
    /// Shortcut widget
    shortcuts: Widget,
}

impl<'a: 'static> BrowserPage<'a> {
    pub fn new(tx: UnboundedSender<Message>) -> Self {
        Self {
            props: BrowserPageProps::default(),
            sections: SectionGroup::default()
                .section(Browser::new(tx.clone()).to_widget(tx.clone()))
                .to_widget(tx.clone())
                .on_update(|state| {
                    let props = BrowserPageProps::from(state);
                    SectionGroupProps::default()
                        .handle_keys(props.handle_keys)
                        .to_boxed_any()
                        .into()
                }),
            shortcuts: Shortcuts::default()
                .to_widget(tx.clone())
                .on_update(|state| {
                    ShortcutsProps::default()
                        .shortcuts(&BrowserPageProps::from(state).shortcuts)
                        .to_boxed_any()
                        .into()
                }),
        }
    }
}

impl<'a: 'static> View for BrowserPage<'a> {
    type Message = Message;
    type State = State;

    fn handle_event(&mut self, key: Key) -> Option<Self::Message> {
        self.sections.handle_event(key);

        if self.props.handle_keys {
            return match key {
                Key::Esc | Key::Ctrl('c') => Some(Message::Exit { selection: None }),
                Key::Char('?') => Some(Message::OpenHelp),
                _ => None,
            };
        }

        None
    }

    fn update(&mut self, state: &Self::State, props: Option<ViewProps>) {
        if let Some(props) = props.and_then(|props| props.inner::<BrowserPageProps>()) {
            self.props = props;
        } else {
            self.props = BrowserPageProps::from(state);
        }

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

        // TODO: Find better solution
        if page_size != self.props.page_size {
            self.sections.send(Message::BrowserPageSize(page_size));
        }
    }
}

#[derive(Clone)]
pub struct SearchProps {}

pub struct Search {
    /// Internal props
    props: SearchProps,
    /// Search input field
    input: Widget,
}

impl Search {
    fn new(tx: UnboundedSender<Message>) -> Self
    where
        Self: Sized,
    {
        Self {
            props: SearchProps {},
            input: TextField::default()
                .to_widget(tx.clone())
                .on_event(|s, _| {
                    Some(Message::UpdateSearch {
                        value: s.and_then(|i| i.unwrap_string()).unwrap_or_default(),
                    })
                })
                .on_update(|state: &State| {
                    TextFieldProps::default()
                        .text(&state.browser.search.read().to_string())
                        .title("Search")
                        .inline(true)
                        .to_boxed_any()
                        .into()
                }),
        }
    }
}

impl View for Search {
    type Message = Message;
    type State = State;

    fn handle_event(&mut self, key: termion::event::Key) -> Option<Self::Message> {
        match key {
            Key::Esc => Some(Message::CloseSearch),
            Key::Char('\n') => Some(Message::ApplySearch),
            _ => {
                self.input.handle_event(key);
                None
            }
        }
    }

    fn update(&mut self, state: &Self::State, props: Option<ViewProps>) {
        if let Some(props) = props.and_then(|props| props.inner::<SearchProps>()) {
            self.props = props;
        }

        self.input.update(state);
    }

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let layout = Layout::horizontal(Constraint::from_mins([0]))
            .horizontal_margin(1)
            .split(props.area);

        self.input.render(frame, RenderProps::from(layout[0]));
    }
}

#[derive(Clone, Default)]
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

pub struct HelpPage<'a> {
    /// Internal props
    props: HelpPageProps<'a>,
    /// Content widget
    content: Widget,
    /// Shortcut widget
    shortcuts: Widget,
}

impl<'a: 'static> HelpPage<'a> {
    pub fn new(tx: UnboundedSender<Message>) -> Self
    where
        Self: Sized,
    {
        Self {
            props: HelpPageProps::default(),
            content: Container::default()
                .header(Header::default().to_widget(tx.clone()).on_update(|_| {
                    HeaderProps::default()
                        .columns([Column::new(" Help ", Constraint::Fill(1))].to_vec())
                        .to_boxed_any()
                        .into()
                }))
                .content(
                    Paragraph::default()
                        .to_widget(tx.clone())
                        .on_event(|s, _| {
                            Some(Message::ScrollHelp {
                                progress: s.and_then(|p| p.unwrap_usize()).unwrap_or_default(),
                            })
                        })
                        .on_update(|state| {
                            let props = HelpPageProps::from(state);

                            ParagraphProps::default()
                                .text(&help_text())
                                .page_size(props.page_size)
                                .to_boxed_any()
                                .into()
                        }),
                )
                .footer(Footer::default().to_widget(tx.clone()).on_update(|state| {
                    let props = HelpPageProps::from(state);

                    FooterProps::default()
                        .columns(
                            [
                                Column::new(Text::raw(""), Constraint::Fill(1)),
                                Column::new(
                                    span::default(&format!("{}%", props.help_progress)).dim(),
                                    Constraint::Min(4),
                                ),
                            ]
                            .to_vec(),
                        )
                        .to_boxed_any()
                        .into()
                }))
                .to_widget(tx.clone()),
            shortcuts: Shortcuts::default()
                .to_widget(tx.clone())
                .on_update(|state| {
                    ShortcutsProps::default()
                        .shortcuts(&HelpPageProps::from(state).shortcuts)
                        .to_boxed_any()
                        .into()
                }),
        }
    }
}

impl<'a: 'static> View for HelpPage<'a> {
    type Message = Message;
    type State = State;

    fn handle_event(&mut self, key: termion::event::Key) -> Option<Self::Message> {
        match key {
            Key::Esc | Key::Ctrl('c') => Some(Message::Exit { selection: None }),
            Key::Char('?') => Some(Message::LeavePage),
            _ => {
                self.content.handle_event(key);
                None
            }
        }
    }

    fn update(&mut self, state: &Self::State, props: Option<ViewProps>) {
        if let Some(props) = props.and_then(|props| props.inner::<HelpPageProps>()) {
            self.props = props;
        } else {
            self.props = HelpPageProps::from(state);
        }

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

        // TODO: Find better solution
        if page_size != self.props.page_size {
            self.content.send(Message::HelpPageSize(page_size));
        }
    }
}

fn browse_footer<'a>(props: &BrowserProps<'a>, selected: Option<usize>) -> Vec<Column<'a>> {
    let filter = PatchItemFilter::from_str(&props.search).unwrap_or_default();

    let search = Line::from(vec![
        span::default(" Search ").cyan().dim().reversed(),
        span::default(" "),
        span::default(&props.search.to_string()).gray().dim(),
    ]);

    let draft = Line::from(vec![
        span::default(&props.stats.get("Draft").unwrap_or(&0).to_string()).dim(),
        span::default(" Draft").dim(),
    ]);

    let open = Line::from(vec![
        span::positive(&props.stats.get("Open").unwrap_or(&0).to_string()).dim(),
        span::default(" Open").dim(),
    ]);

    let merged = Line::from(vec![
        span::default(&props.stats.get("Merged").unwrap_or(&0).to_string())
            .magenta()
            .dim(),
        span::default(" Merged").dim(),
    ]);

    let archived = Line::from(vec![
        span::default(&props.stats.get("Archived").unwrap_or(&0).to_string())
            .yellow()
            .dim(),
        span::default(" Archived").dim(),
    ]);

    let sum = Line::from(vec![
        span::default("Σ ").dim(),
        span::default(&props.patches.len().to_string()).dim(),
    ]);

    let progress = selected
        .map(|selected| TableUtils::progress(selected, props.patches.len(), props.page_size))
        .unwrap_or_default();
    let progress = span::default(&format!("{}%", progress)).dim();

    match filter.status() {
        Some(state) => {
            let block = match state {
                Status::Draft => draft,
                Status::Open => open,
                Status::Merged => merged,
                Status::Archived => archived,
            };

            vec![
                Column::new(Text::from(search), Constraint::Fill(1)),
                Column::new(
                    Text::from(block.clone()),
                    Constraint::Min(block.width() as u16),
                ),
                Column::new(Text::from(progress), Constraint::Min(4)),
            ]
        }
        None => vec![
            Column::new(Text::from(search), Constraint::Fill(1)),
            Column::new(
                Text::from(draft.clone()),
                Constraint::Min(draft.width() as u16),
            ),
            Column::new(
                Text::from(open.clone()),
                Constraint::Min(open.width() as u16),
            ),
            Column::new(
                Text::from(merged.clone()),
                Constraint::Min(merged.width() as u16),
            ),
            Column::new(
                Text::from(archived.clone()),
                Constraint::Min(archived.width() as u16),
            ),
            Column::new(Text::from(sum.clone()), Constraint::Min(sum.width() as u16)),
            Column::new(Text::from(progress), Constraint::Min(4)),
        ],
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
                Span::raw("Select patch (if --mode id)").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "enter")).gray(),
                Span::raw(" "),
                Span::raw("Show patch").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "c")).gray(),
                Span::raw(" "),
                Span::raw("Checkout patch").gray().dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "d")).gray(),
                Span::raw(" "),
                Span::raw("Show patch diff").gray().dim(),
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
                Span::raw("is:<state> | is:authored | authors:[<did>, <did>] | <search>")
                    .gray()
                    .dim(),
            ]),
            Line::from(vec![
                Span::raw(format!("{key:>10}", key = "Example")).gray(),
                Span::raw(" "),
                Span::raw("is:open is:authored improve").gray().dim(),
            ]),
            Line::raw(""),
            Line::raw(""),
        ]
        .to_vec(),
    )
}
