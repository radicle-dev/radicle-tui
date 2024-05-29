#[path = "select/ui.rs"]
mod ui;

use std::str::FromStr;

use anyhow::Result;

use radicle::patch::PatchId;
use radicle::storage::git::Repository;
use radicle::Profile;

use radicle_tui as tui;

use tui::cob::patch;
use tui::store;
use tui::ui::items::{Filter, PatchItem, PatchItemFilter};
use tui::ui::widget::window::{Window, WindowProps};
use tui::ui::widget::ToWidget;

use tui::{BoxedAny, Channel, Exit, PageStack};

use self::ui::BrowserPage;
use self::ui::HelpPage;

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
pub enum Page {
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
    pages: PageStack<Page>,
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
            pages: PageStack::new(vec![Page::Browse]),
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
                self.pages.push(Page::Help);
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
            .page(
                Page::Browse,
                BrowserPage::new(tx.clone()).to_widget(tx.clone()),
            )
            .page(Page::Help, HelpPage::new(tx.clone()).to_widget(tx.clone()))
            .to_widget(tx.clone())
            .on_update(|state| {
                WindowProps::default()
                    .current_page(state.pages.peek().unwrap_or(&Page::Browse).clone())
                    .to_boxed_any()
                    .into()
            });

        tui::run(channel, state, window).await
    }
}
