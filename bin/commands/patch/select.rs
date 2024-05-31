#[path = "select/ui.rs"]
mod ui;

use std::str::FromStr;

use anyhow::Result;

use radicle::patch::PatchId;
use radicle::storage::git::Repository;
use radicle::Profile;

use radicle_tui as tui;

use ratatui::layout::Constraint;
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};

use termion::event::Key;
use tui::cob::patch;
use tui::store;
use tui::ui::items::{Filter, PatchItem, PatchItemFilter};
use tui::ui::span;
use tui::ui::widget::container::{Column, Container, Footer, FooterProps, Header, HeaderProps};
use tui::ui::widget::text::{Paragraph, ParagraphProps};
use tui::ui::widget::window::{Page, PageProps, Shortcuts, ShortcutsProps, Window, WindowProps};
use tui::ui::widget::{ToWidget, Widget};

use tui::{BoxedAny, Channel, Exit, PageStack};

use self::ui::{Browser, BrowserProps};

use super::common::Mode;

type Selection = tui::Selection<PatchId>;

pub struct Context {
    pub profile: Profile,
    pub repository: Repository,
    pub mode: Mode,
    pub filter: patch::Filter,
}

pub struct App {
    context: Context,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum AppPage {
    Browse,
    Help,
}

#[derive(Clone, Debug)]
pub struct BrowserState {
    items: Vec<PatchItem>,
    selected: Option<usize>,
    filter: PatchItemFilter,
    search: store::StateValue<String>,
    page_size: usize,
    show_search: bool,
}

impl BrowserState {
    pub fn patches(&self) -> Vec<PatchItem> {
        self.items
            .iter()
            .filter(|patch| self.filter.matches(patch))
            .cloned()
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct HelpState {
    progress: usize,
    page_size: usize,
}

#[derive(Clone, Debug)]
pub struct State {
    mode: Mode,
    pages: PageStack<AppPage>,
    browser: BrowserState,
    help: HelpState,
}

impl TryFrom<&Context> for State {
    type Error = anyhow::Error;

    fn try_from(context: &Context) -> Result<Self, Self::Error> {
        let patches = patch::all(&context.profile, &context.repository)?;
        let search = store::StateValue::new(context.filter.to_string());
        let filter = PatchItemFilter::from_str(&context.filter.to_string()).unwrap_or_default();

        // Convert into UI items
        let mut items = vec![];
        for patch in patches {
            if let Ok(item) = PatchItem::new(&context.profile, &context.repository, patch.clone()) {
                items.push(item);
            }
        }
        items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(Self {
            mode: context.mode.clone(),
            pages: PageStack::new(vec![AppPage::Browse]),
            browser: BrowserState {
                items,
                selected: Some(0),
                filter,
                search,
                show_search: false,
                page_size: 1,
            },
            help: HelpState {
                progress: 0,
                page_size: 1,
            },
        })
    }
}

pub enum Message {
    Exit { selection: Option<Selection> },
    Select { selected: Option<usize> },
    BrowserPageSize(usize),
    HelpPageSize(usize),
    OpenSearch,
    UpdateSearch { value: String },
    ApplySearch,
    CloseSearch,
    OpenHelp,
    LeavePage,
    ScrollHelp { progress: usize },
}

impl store::State<Selection> for State {
    type Message = Message;

    fn update(&mut self, message: Message) -> Option<Exit<Selection>> {
        match message {
            Message::Exit { selection } => Some(Exit { value: selection }),
            Message::Select { selected } => {
                self.browser.selected = selected;
                None
            }
            Message::BrowserPageSize(size) => {
                self.browser.page_size = size;
                None
            }
            Message::HelpPageSize(size) => {
                self.help.page_size = size;
                None
            }
            Message::OpenSearch => {
                self.browser.show_search = true;
                None
            }
            Message::UpdateSearch { value } => {
                self.browser.search.write(value);
                self.browser.filter =
                    PatchItemFilter::from_str(&self.browser.search.read()).unwrap_or_default();

                if let Some(selected) = self.browser.selected {
                    if selected > self.browser.patches().len() {
                        self.browser.selected = Some(0);
                    }
                }

                None
            }
            Message::ApplySearch => {
                self.browser.search.apply();
                self.browser.show_search = false;
                None
            }
            Message::CloseSearch => {
                self.browser.search.reset();
                self.browser.show_search = false;
                self.browser.filter =
                    PatchItemFilter::from_str(&self.browser.search.read()).unwrap_or_default();

                None
            }
            Message::OpenHelp => {
                self.pages.push(AppPage::Help);
                None
            }
            Message::LeavePage => {
                self.pages.pop();
                None
            }
            Message::ScrollHelp { progress } => {
                self.help.progress = progress;
                None
            }
        }
    }

    fn tick(&self) {}
}

impl App {
    pub fn new(context: Context) -> Self {
        Self { context }
    }

    pub async fn run(&self) -> Result<Option<Selection>> {
        let channel = Channel::default();
        let state = State::try_from(&self.context)?;
        let tx = channel.tx.clone();

        let window = Window::default()
            .page(AppPage::Browse, browser_page(&state, &channel))
            .page(AppPage::Help, help_page(&state, &channel))
            .to_widget(tx.clone())
            .on_update(|state| {
                WindowProps::default()
                    .current_page(state.pages.peek().unwrap_or(&AppPage::Browse).clone())
                    .to_boxed_any()
                    .into()
            });

        tui::run(channel, state, window).await
    }
}

fn browser_page(_state: &State, channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    Page::default()
        .content(
            Browser::new(tx.clone())
                .to_widget(tx.clone())
                .on_update(|state| BrowserProps::from(state).to_boxed_any().into()),
        )
        .shortcuts(
            Shortcuts::default()
                .to_widget(tx.clone())
                .on_update(|state| {
                    let shortcuts = if state.browser.show_search {
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
                    };

                    ShortcutsProps::default()
                        .shortcuts(&shortcuts)
                        .to_boxed_any()
                        .into()
                }),
        )
        .to_widget(tx.clone())
        .on_event(|key, _, props| {
            let default = PageProps::default();
            let props = props
                .and_then(|props| props.inner_ref::<PageProps>())
                .unwrap_or(&default);

            if props.handle_keys {
                match key {
                    Key::Esc | Key::Ctrl('c') => Some(Message::Exit { selection: None }),
                    Key::Char('?') => Some(Message::OpenHelp),
                    _ => None,
                }
            } else {
                None
            }
        })
        .on_update(|state: &State| {
            PageProps::default()
                .page_size(state.browser.page_size)
                .handle_keys(!state.browser.show_search)
                .to_boxed_any()
                .into()
        })
        .on_render(|props, render| {
            let default = PageProps::default();
            let props = props
                .and_then(|props| props.inner_ref::<PageProps>())
                .unwrap_or(&default);
            let page_size = render.area.height.saturating_sub(6) as usize;

            if page_size != props.page_size {
                return Some(Message::BrowserPageSize(page_size));
            }
            None
        })
}

fn help_page(_state: &State, channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    Page::default()
        .content(
            Container::default()
                .header(Header::default().to_widget(tx.clone()).on_update(|_| {
                    HeaderProps::default()
                        .columns([Column::new(" Help ", Constraint::Fill(1))].to_vec())
                        .to_boxed_any()
                        .into()
                }))
                .content(
                    Paragraph::default()
                        .to_widget(tx.clone())
                        .on_event(|_, s, _| {
                            Some(Message::ScrollHelp {
                                progress: s.and_then(|p| p.unwrap_usize()).unwrap_or_default(),
                            })
                        })
                        .on_update(|state: &State| {
                            ParagraphProps::default()
                                .text(&help_text())
                                .page_size(state.help.page_size)
                                .to_boxed_any()
                                .into()
                        }),
                )
                .footer(Footer::default().to_widget(tx.clone()).on_update(|state| {
                    FooterProps::default()
                        .columns(
                            [
                                Column::new(Text::raw(""), Constraint::Fill(1)),
                                Column::new(
                                    span::default(&format!("{}%", state.help.progress)).dim(),
                                    Constraint::Min(4),
                                ),
                            ]
                            .to_vec(),
                        )
                        .to_boxed_any()
                        .into()
                }))
                .to_widget(tx.clone()),
        )
        .shortcuts(Shortcuts::default().to_widget(tx.clone()).on_update(|_| {
            ShortcutsProps::default()
                .shortcuts(&[("?", "close")])
                .to_boxed_any()
                .into()
        }))
        .to_widget(tx.clone())
        .on_event(|key, _, _| match key {
            Key::Esc | Key::Ctrl('c') => Some(Message::Exit { selection: None }),
            Key::Char('?') => Some(Message::LeavePage),
            _ => None,
        })
        .on_update(|state: &State| {
            PageProps::default()
                .page_size(state.help.page_size)
                .handle_keys(true)
                .to_boxed_any()
                .into()
        })
        .on_render(|props, render| {
            let default = PageProps::default();
            let props = props
                .and_then(|props| props.inner_ref::<PageProps>())
                .unwrap_or(&default);
            let page_size = render.area.height.saturating_sub(6) as usize;

            if page_size != props.page_size {
                return Some(Message::HelpPageSize(page_size));
            }
            None
        })
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
