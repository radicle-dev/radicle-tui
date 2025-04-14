use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::sync::{Arc, Mutex};

use anyhow::Result;

use radicle::cob::ObjectId;
use radicle::issue::{Issue, State};
use serde::{Deserialize, Serialize};

use termion::event::Key;

use ratatui::layout::{Constraint, Position};
use ratatui::style::{Style, Stylize};
use ratatui::text::Text;
use ratatui::{Frame, Viewport};

use radicle::identity::RepoId;
use radicle::patch::{PatchId, Review, Revision, RevisionId};
use radicle::storage::ReadStorage;
use radicle::{Profile, Storage};

use radicle_tui as tui;

use tui::store;
use tui::ui::im::widget::{PanesState, TableState, TextViewState, Window};
use tui::ui::im::{Borders, Context, Show, Ui};
use tui::ui::span;
use tui::ui::Column;
use tui::{Channel, Exit};

use crate::git::HunkState;
use crate::state::{self, FileIdentifier, FileStore, ReadState, WriteState};
use crate::ui::format;
use crate::ui::items::StatefulHunkItem;
use crate::ui::items::{HunkItem, IssueItem};
use crate::ui::layout;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Args(String);

#[derive(Clone, Debug)]
pub struct Response {
    pub state: AppState,
}

pub struct Tui {
    pub profile: Profile,
    pub rid: RepoId,
    pub issues: Vec<(ObjectId, Issue)>,
}

impl Tui {
    #[allow(clippy::too_many_arguments)]
    pub fn new(profile: Profile, rid: RepoId, issues: Vec<(ObjectId, Issue)>) -> Self {
        Self {
            profile,
            rid,
            issues,
        }
    }

    pub async fn run(self) -> Result<Option<Response>> {
        let viewport = Viewport::Fullscreen;
        let channel = Channel::default();

        let identifier = FileIdentifier::new("issue", "board", &self.rid, None);
        let store = FileStore::new(identifier)?;

        let state = store
            .read()
            .map(|bytes| state::from_json::<AppState>(&bytes).unwrap_or(AppState::new(self.rid)))
            .unwrap_or(AppState::new(self.rid));

        let app = App::new(self.profile, self.issues, state)?;
        let response = tui::im(app, viewport, channel).await?;

        if let Some(response) = response.as_ref() {
            store.write(&state::to_json(&response.state)?)?;
        }

        Ok(response)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum AppPage {
    Main,
    Help,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Lane {
    Backlog,
    Todo,
    InProgress,
    Done,
}

impl Display for Lane {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Lane::Backlog => f.write_str("Backlog"),
            Lane::Todo => f.write_str("Todo"),
            Lane::InProgress => f.write_str("In Progess"),
            Lane::Done => f.write_str("Done"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppState {
    /// The repository to operate on.
    rid: RepoId,
    /// Current app page.
    page: AppPage,
    /// State of panes widget on the main page.
    panes: PanesState,
    /// Selected issue per lane.
    issues: HashMap<Lane, TableState>,
    /// State of text view widget on the help page.
    help: TextViewState,
}

impl AppState {
    pub fn new(rid: RepoId) -> Self {
        Self {
            rid,
            page: AppPage::Main,
            panes: PanesState::new(2, Some(0)),
            issues: HashMap::from([(Lane::Backlog, TableState::new(Some(0)))]),
            help: TextViewState::new(Position::default()),
        }
    }

    pub fn selected_issue(&self, lane: &Lane) -> Option<usize> {
        self.issues.get(lane).map(|state| state.selected())?
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    PanesChanged { state: PanesState },
    LaneChanged { lane: Lane, state: TableState },
    ShowHelp,
    HelpChanged { state: TextViewState },
    Quit,
}

#[derive(Clone)]
pub struct App {
    /// All issues, spread over pre-defined lanes.
    lanes: Arc<Mutex<HashMap<Lane, Vec<IssueItem>>>>,
    /// The app state.
    state: Arc<Mutex<AppState>>,
}

impl App {
    pub fn new(
        profile: Profile,
        issues: Vec<(ObjectId, Issue)>,
        state: AppState,
    ) -> Result<Self, anyhow::Error> {
        let mut backlog = vec![];
        for (oid, issue) in issues {
            if *issue.state() == State::Open {
                backlog.push(IssueItem::new(&profile, (oid, issue))?);
            }
        }

        let mut lanes = HashMap::new();
        lanes.insert(Lane::Backlog, backlog);

        Ok(Self {
            lanes: Arc::new(Mutex::new(lanes)),
            state: Arc::new(Mutex::new(state)),
        })
    }
}

impl App {
    fn show_lane(&self, ui: &mut Ui<Message>, frame: &mut Frame, lane: Lane) {
        let lanes = self.lanes.lock().unwrap();
        let state = self.state.lock().unwrap();

        let empty: Vec<IssueItem> = vec![];
        let issues = lanes.get(&lane).unwrap_or(&empty);

        let header = [Column::new(format!(" {lane} "), Constraint::Fill(1))].to_vec();
        let columns = [
            Column::new("", Constraint::Length(3)),
            Column::new("", Constraint::Length(7)),
            Column::new("", Constraint::Fill(1)),
        ]
        .to_vec();

        let mut selected = state.selected_issue(&lane);

        let table = ui.headered_table(frame, &mut selected, &issues, header, columns, None);
        if table.changed {
            ui.send_message(Message::LaneChanged {
                lane,
                state: TableState::new(selected),
            })
        }
    }

    fn show_footer(&self, ui: &mut Ui<Message>, frame: &mut Frame) {
        ui.shortcuts(frame, &[("?", "help"), ("q", "quit")], '∙');

        if ui.input_global(|key| key == Key::Char('?')) {
            ui.send_message(Message::ShowHelp);
        }
    }
}

impl Show<Message> for App {
    fn show(&self, ctx: &Context<Message>, frame: &mut Frame) -> Result<(), anyhow::Error> {
        Window::default().show(ctx, |ui| {
            let page = {
                let state = self.state.lock().unwrap();
                state.page.clone()
            };

            match page {
                AppPage::Main => {
                    let (mut focus, count) = {
                        let state = self.state.lock().unwrap();
                        (state.panes.focus(), state.panes.len())
                    };

                    ui.layout(layout::page(), Some(0), |ui| {
                        let group = ui.panes(layout::columns(4), &mut focus, |ui| {
                            self.show_lane(ui, frame, Lane::Backlog);
                            self.show_lane(ui, frame, Lane::Todo);
                            self.show_lane(ui, frame, Lane::InProgress);
                            self.show_lane(ui, frame, Lane::Done);
                        });
                        if group.response.changed {
                            ui.send_message(Message::PanesChanged {
                                state: PanesState::new(count, focus),
                            });
                        }

                        // self.show_context_bar(ui, frame);
                        self.show_footer(ui, frame);
                    });
                }
                AppPage::Help => {}
            }

            if ui.input_global(|key| key == Key::Char('q')) {
                ui.send_message(Message::Quit);
            }
        });
        Ok(())
    }
}

impl store::Update<Message> for App {
    type Return = Response;

    fn update(&mut self, message: Message) -> Option<Exit<Self::Return>> {
        log::info!("Received message: {:?}", message);

        match message {
            Message::ShowHelp => {
                let mut state = self.state.lock().unwrap();
                state.page = AppPage::Help;
                None
            }
            Message::HelpChanged { state } => {
                let mut app_state = self.state.lock().unwrap();
                app_state.help = state;
                None
            }
            Message::PanesChanged { state } => {
                let mut app_state = self.state.lock().unwrap();
                app_state.panes = state;
                None
            }
            Message::LaneChanged { lane, state } => {
                let mut app_state = self.state.lock().unwrap();
                app_state.issues.insert(lane, state);
                None
            }
            Message::Quit => {
                let state = self.state.lock().unwrap();
                Some(Exit {
                    value: Some(Response {
                        state: state.clone(),
                    }),
                })
            }
        }
    }
}
