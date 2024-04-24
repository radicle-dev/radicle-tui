#[path = "select/ui.rs"]
mod ui;

use std::str::FromStr;

use anyhow::Result;

use radicle::issue::IssueId;
use radicle::storage::git::Repository;
use radicle::Profile;

use radicle_tui as tui;

use tui::cob::issue;
use tui::store::StateValue;
use tui::task;
use tui::task::Interrupted;
use tui::terminal::Backend;
use tui::ui::items::{Filter, IssueItem, IssueItemFilter};
use tui::ui::widget::{Properties, View, Window, WindowProps};
use tui::ui::Frontend;
use tui::Exit;
use tui::{store, PageStack};

use self::ui::{BrowsePage, HelpPage};

use super::common::Mode;

type Selection = tui::Selection<IssueId>;

pub struct Context {
    pub profile: Profile,
    pub repository: Repository,
    pub mode: Mode,
    pub filter: issue::Filter,
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
    items: Vec<IssueItem>,
    selected: Option<usize>,
    filter: IssueItemFilter,
    search: store::StateValue<String>,
    page_size: usize,
    show_search: bool,
}

impl BrowserState {
    pub fn issues(&self) -> Vec<IssueItem> {
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
        let issues = issue::all(&context.profile, &context.repository)?;
        let search = StateValue::new(context.filter.to_string());
        let filter = IssueItemFilter::from_str(&search.read()).unwrap_or_default();

        // Convert into UI items
        let mut items = vec![];
        for issue in issues {
            if let Ok(item) = IssueItem::new(&context.profile, issue.clone()) {
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

pub enum Action {
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
    type Action = Action;

    fn handle_action(&mut self, action: Action) -> Option<Exit<Selection>> {
        match action {
            Action::Exit { selection } => Some(Exit { value: selection }),
            Action::Select { selected } => {
                self.browser.selected = selected;
                None
            }
            Action::BrowserPageSize(size) => {
                self.browser.page_size = size;
                None
            }
            Action::HelpPageSize(size) => {
                self.help.page_size = size;
                None
            }
            Action::OpenSearch => {
                self.browser.show_search = true;
                None
            }
            Action::UpdateSearch { value } => {
                self.browser.search.write(value);
                self.browser.filter =
                    IssueItemFilter::from_str(&self.browser.search.read()).unwrap_or_default();

                None
            }
            Action::ApplySearch => {
                self.browser.search.apply();
                self.browser.show_search = false;
                None
            }
            Action::CloseSearch => {
                self.browser.search.reset();
                self.browser.show_search = false;
                self.browser.filter =
                    IssueItemFilter::from_str(&self.browser.search.read()).unwrap_or_default();

                None
            }
            Action::OpenHelp => {
                self.pages.push(Page::Help);
                None
            }
            Action::LeavePage => {
                self.pages.pop();
                None
            }
            Action::ScrollHelp { progress } => {
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
        let (terminator, mut interrupt_rx) = task::create_termination();
        let (store, state_rx) = store::Store::<Action, State, Selection>::new();
        let (frontend, action_tx, action_rx) = Frontend::new();
        let state = State::try_from(&self.context)?;

        let window: Window<Backend, State, Action, Page> = Window::new(&state, action_tx.clone())
            .page(
                Page::Browse,
                BrowsePage::new(&state, action_tx.clone()).to_boxed(),
            )
            .page(
                Page::Help,
                HelpPage::new(&state, action_tx.clone()).to_boxed(),
            )
            .on_update(|state| {
                WindowProps::default()
                    .current_page(state.pages.peek().unwrap_or(&Page::Browse).clone())
                    .to_boxed()
            });

        tokio::try_join!(
            store.main_loop(state, terminator, action_rx, interrupt_rx.resubscribe()),
            frontend.main_loop(Some(window), state_rx, interrupt_rx.resubscribe()),
        )?;

        if let Ok(reason) = interrupt_rx.recv().await {
            match reason {
                Interrupted::User { payload } => Ok(payload),
                Interrupted::OsSignal => anyhow::bail!("exited because of an os sig int"),
            }
        } else {
            anyhow::bail!("exited because of an unexpected error");
        }
    }
}
