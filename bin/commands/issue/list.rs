use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use serde::Serialize;

use anyhow::{bail, Result};

use ratatui::layout::{Alignment, Constraint, Layout, Position};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::{Frame, Viewport};

use radicle::cob::thread::CommentId;
use radicle::issue::IssueId;
use radicle::storage::git::Repository;
use radicle::Profile;

use radicle_tui as tui;

use tui::event::Key;
use tui::store;
use tui::task::EmptyProcessors;
use tui::ui;
use tui::ui::layout::Spacing;
use tui::ui::span;
use tui::ui::widget::{
    Borders, Column, ContainerState, TableState, TextEditState, TextViewState, TreeState, Window,
};
use tui::ui::{BufferedValue, Show, ToRow, Ui};
use tui::{Channel, Exit};

use crate::cob::issue;
use crate::settings::{self, ThemeBundle, ThemeMode};
use crate::ui::items::filter::Filter;
use crate::ui::items::issue::filter::IssueFilter;
use crate::ui::items::issue::Issue;
use crate::ui::items::HasId;
use crate::ui::{format, TerminalInfo};

type Selection = tui::Selection<IssueOperation>;

/// The selected issue operation returned by the operation
/// selection widget.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum IssueOperation {
    Edit {
        id: IssueId,
        comment_id: Option<CommentId>,
        search: String,
    },
    Show {
        id: IssueId,
    },
    Close {
        id: IssueId,
        search: String,
    },
    Solve {
        id: IssueId,
        search: String,
    },
    Reopen {
        id: IssueId,
        search: String,
    },
    Comment {
        id: IssueId,
        reply_to: Option<CommentId>,
        search: String,
    },
}

const HELP: &str = r#"# Generic keybindings

`↑,k`:      move cursor one line up
`↓,j:       move cursor one line down
`PageUp`:   move cursor one page up
`PageDown`: move cursor one page down
`Home`:     move cursor to the first line
`End`:      move cursor to the last line
`Tab`:      focus next section
`BackTab`:  focus previous section
`Esc`:      Cancel
`q`:        Quit

# Specific keybindings

`/`:        Search
`Enter`:    Show issue
`e`:        Edit issue
`s`:        Solve issue
`l`:        Close issue
`o`:        Re-open issue
`c`:        Reply to comment
`p`:        Toggle issue preview
`?`:        Show help"#;

pub struct Context {
    pub profile: Profile,
    pub repository: Repository,
    pub filter: IssueFilter,
    pub search: Option<String>,
    pub issue: Option<IssueId>,
    pub comment: Option<CommentId>,
}

pub(crate) struct Tui {
    pub(crate) context: Context,
    pub(crate) terminal_info: TerminalInfo,
}

impl Tui {
    pub fn new(context: Context, terminal_info: TerminalInfo) -> Self {
        Self {
            context,
            terminal_info,
        }
    }

    pub async fn run(&self) -> Result<Option<Selection>> {
        let viewport = Viewport::Inline(20);
        let channel = Channel::default();
        let state = App::try_from((&self.context, &self.terminal_info))?;

        tui::im(state, viewport, channel, EmptyProcessors::new()).await
    }

    pub fn context(&self) -> &Context {
        &self.context
    }
}

mod state {
    use super::*;
    use crate::ui::items::CommentItem;

    #[derive(Clone, Debug)]
    pub(crate) enum Page {
        Main,
        Help,
    }

    #[derive(Clone, Default, Debug, Eq, PartialEq)]
    pub(crate) enum Section {
        #[default]
        Browser,
        Issue,
        Comment,
    }

    impl TryFrom<usize> for Section {
        type Error = anyhow::Error;

        fn try_from(value: usize) -> Result<Self, Self::Error> {
            match value {
                0 => Ok(Section::Browser),
                1 => Ok(Section::Issue),
                2 => Ok(Section::Comment),
                _ => bail!("Unknown section index: {}", value),
            }
        }
    }

    #[derive(Clone, Debug)]
    pub(crate) struct Browser {
        pub(crate) issues: TableState,
        pub(crate) search: BufferedValue<TextEditState>,
        pub(crate) show_search: bool,
    }

    impl Browser {
        pub fn selected(&self) -> Option<usize> {
            self.issues.selected()
        }
    }

    #[derive(Clone, Debug)]
    pub(crate) struct Preview {
        /// If preview is visible.
        pub(crate) show: bool,
        /// Currently selected issue item.
        pub(crate) issue: Option<Issue>,
        /// Tree selection per issue.
        pub(crate) selected_comments: HashMap<IssueId, Vec<CommentId>>,
        /// State of currently selected comment
        pub(crate) comment: TextViewState,
    }

    impl Preview {
        pub fn root_comments(&self) -> Vec<CommentItem> {
            self.issue
                .as_ref()
                .map(|item| item.root_comments())
                .unwrap_or_default()
        }

        pub fn selected_comment(&self) -> Option<&CommentItem> {
            self.issue.as_ref().and_then(|item| {
                self.selected_comments
                    .get(&item.id)
                    .and_then(|selection| selection.last().copied())
                    .and_then(|comment_id| item.comments.iter().find(|item| item.id == comment_id))
            })
        }

        pub fn selected_comment_ids(&self) -> Vec<String> {
            self.issue
                .as_ref()
                .and_then(|item| self.selected_comments.get(&item.id))
                .map(|selected| selected.iter().map(|oid| oid.to_string()).collect())
                .unwrap_or_default()
        }

        pub fn opened_comments(&self) -> HashSet<Vec<String>> {
            let mut opened = HashSet::new();
            if let Some(item) = &self.issue {
                for comment in item.root_comments() {
                    append_opened(&mut opened, vec![], comment.clone());
                }
            }
            opened
        }
    }

    fn append_opened(all: &mut HashSet<Vec<String>>, path: Vec<String>, comment: CommentItem) {
        all.insert([path.clone(), [comment.id.to_string()].to_vec()].concat());

        for reply in comment.replies {
            append_opened(
                all,
                [path.clone(), [comment.id.to_string()].to_vec()].concat(),
                reply,
            );
        }
    }
}

#[derive(Clone, Debug)]
pub enum Change {
    Page { page: state::Page },
    Section { state: ContainerState },
    Issue { state: TableState },
    Comment { state: TreeState<String> },
    CommentBody { state: TextViewState },
    ShowSearch { state: bool, apply: bool },
    ShowPreview { state: bool },
    Search { state: BufferedValue<TextEditState> },
    Help { state: TextViewState },
}

#[derive(Clone, Debug)]
pub enum Message {
    Changed(Change),
    Exit { operation: Option<IssueOperation> },
    Quit,
}

#[derive(Clone, Debug)]
pub struct AppState {
    page: state::Page,
    sections: ContainerState,
    browser: state::Browser,
    preview: state::Preview,
    help: TextViewState,
    filter: IssueFilter,
}

#[derive(Clone, Debug)]
pub struct App {
    issues: Arc<Mutex<Vec<Issue>>>,
    state: AppState,
}

impl TryFrom<(&Context, &TerminalInfo)> for App {
    type Error = anyhow::Error;

    fn try_from(value: (&Context, &TerminalInfo)) -> Result<Self, Self::Error> {
        let (context, terminal_info) = value;
        let settings = settings::Settings::default();

        let issues = issue::all(&context.profile, &context.repository)?;
        let search =
            BufferedValue::new(context.search.clone().unwrap_or(context.filter.to_string()));
        let filter = IssueFilter::from_str(&search.read()).unwrap_or_default();

        let default_bundle = ThemeBundle::default();
        let theme_bundle = settings.theme.active_bundle().unwrap_or(&default_bundle);
        let _theme = match settings.theme.mode() {
            ThemeMode::Auto => {
                if terminal_info.is_dark() {
                    theme_bundle.dark.clone()
                } else {
                    theme_bundle.light.clone()
                }
            }
            ThemeMode::Light => theme_bundle.light.clone(),
            ThemeMode::Dark => theme_bundle.dark.clone(),
        };

        // Convert into UI items
        let mut issues: Vec<_> = issues
            .into_iter()
            .flat_map(|issue| Issue::new(&context.profile, issue).ok())
            .collect();

        issues.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Pre-select comments per issue. If a comment to pre-select is given,
        // find identifier path needed for selection. Select root comment
        // otherwise.
        let selected_comments: HashMap<_, _> = issues
            .iter()
            .map(|issue| {
                let comment_ids = match context.comment {
                    Some(comment_id) if issue.has_comment(&comment_id) => {
                        issue.path_to_comment(&comment_id).unwrap_or_default()
                    }
                    _ => issue
                        .root_comments()
                        .first()
                        .map(|c| vec![c.id])
                        .unwrap_or_default(),
                };
                (issue.id, comment_ids)
            })
            .collect();

        let browser = state::Browser {
            issues: TableState::new(Some(
                context
                    .issue
                    .and_then(|id| {
                        issues
                            .iter()
                            .filter(|item| filter.matches(item))
                            .position(|item| item.id() == id)
                    })
                    .unwrap_or(0),
            )),
            search: BufferedValue::new(TextEditState {
                text: search.read().clone(),
                cursor: search.read().len(),
            }),
            show_search: false,
        };

        let preview = state::Preview {
            show: true,
            issue: browser
                .selected()
                .and_then(|s| {
                    issues
                        .iter()
                        .filter(|item| filter.matches(item))
                        .collect::<Vec<_>>()
                        .get(s)
                        .cloned()
                })
                .cloned(),
            selected_comments,
            comment: TextViewState::new(Position::default()),
        };

        let section = if context.comment.is_some() {
            state::Section::Issue
        } else {
            state::Section::Browser
        };

        Ok(Self {
            issues: Arc::new(Mutex::new(issues)),
            state: AppState {
                page: state::Page::Main,
                sections: ContainerState::new(3, Some(section as usize)),
                browser,
                preview,
                filter,
                help: TextViewState::new(Position::default()),
            },
        })
    }
}

impl store::Update<Message> for App {
    type Return = Selection;

    fn update(&mut self, message: Message) -> Option<tui::Exit<Selection>> {
        match message {
            Message::Quit => Some(Exit { value: None }),
            Message::Exit { operation } => Some(Exit {
                value: Some(Selection {
                    operation,
                    args: vec![],
                }),
            }),
            Message::Changed(changed) => match changed {
                Change::Page { page } => {
                    self.state.page = page;
                    None
                }
                Change::Section { state } => {
                    self.state.sections = state;
                    None
                }
                Change::Issue { state } => {
                    let issues = self.issues.lock().unwrap();
                    let issues = issues
                        .clone()
                        .into_iter()
                        .filter(|issue| self.state.filter.matches(issue))
                        .collect::<Vec<_>>();

                    self.state.browser.issues = state;
                    self.state.preview.issue = self
                        .state
                        .browser
                        .selected()
                        .and_then(|s| issues.get(s).cloned());
                    self.state.preview.comment = TextViewState::new(Position::default());
                    None
                }
                Change::ShowSearch { state, apply } => {
                    if state {
                        self.state.sections = ContainerState::new(self.state.sections.len(), None);
                        self.state.browser.show_search = true;
                    } else {
                        let issues = self.issues.lock().unwrap();
                        let issues = issues
                            .clone()
                            .into_iter()
                            .filter(|issue| self.state.filter.matches(issue))
                            .collect::<Vec<_>>();

                        self.state.preview.issue = self
                            .state
                            .browser
                            .selected()
                            .and_then(|s| issues.get(s).cloned());
                        self.state.sections =
                            ContainerState::new(self.state.sections.len(), Some(0));
                        self.state.browser.show_search = false;

                        if apply {
                            self.state.browser.search.apply();
                        } else {
                            self.state.browser.search.reset();
                        }

                        self.state.filter =
                            IssueFilter::from_str(&self.state.browser.search.read().text)
                                .unwrap_or_default();
                    }
                    None
                }
                Change::ShowPreview { state } => {
                    self.state.preview.show = state;
                    self.state.sections = ContainerState::new(if state { 3 } else { 1 }, Some(0));
                    None
                }
                Change::Search { state } => {
                    let issues = self.issues.lock().unwrap();
                    let issues = issues
                        .clone()
                        .into_iter()
                        .filter(|issue| self.state.filter.matches(issue))
                        .collect::<Vec<_>>();

                    self.state.browser.search = state.clone();
                    self.state.filter =
                        IssueFilter::from_str(&state.read().text).unwrap_or_default();
                    self.state.browser.issues.select_first();

                    self.state.preview.issue = self
                        .state
                        .browser
                        .selected()
                        .and_then(|s| issues.get(s).cloned());
                    None
                }
                Change::Comment { state } => {
                    if let Some(item) = &self.state.preview.issue {
                        self.state.preview.selected_comments.insert(
                            item.id,
                            state
                                .internal
                                .selected()
                                .iter()
                                .map(|s| CommentId::from_str(s).unwrap())
                                .collect(),
                        );
                    }
                    self.state.preview.comment = TextViewState::new(Position::default());
                    None
                }
                Change::CommentBody { state } => {
                    self.state.preview.comment = state;
                    None
                }
                Change::Help { state } => {
                    self.state.help = state;
                    None
                }
            },
        }
    }
}

impl Show<Message> for App {
    fn show(&self, ctx: &ui::Context<Message>, frame: &mut Frame) -> Result<()> {
        Window::default().show(ctx, |ui| {
            match self.state.page.clone() {
                state::Page::Main => {
                    let show_search = self.state.browser.show_search;
                    let page_focus = if show_search { Some(1) } else { Some(0) };

                    ui.layout(
                        Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]),
                        page_focus,
                        |ui| {
                            let (mut focus, count) =
                                { (self.state.sections.focus(), self.state.sections.len()) };

                            let group = ui.container(
                                ui::Layout::Expandable3 {
                                    left_only: !self.state.preview.show,
                                },
                                &mut focus,
                                |ui| {
                                    self.show_browser(frame, ui);
                                    self.show_issue(frame, ui);
                                    self.show_comment(frame, ui);
                                },
                            );

                            if group.response.changed {
                                ui.send_message(Message::Changed(Change::Section {
                                    state: ContainerState::new(count, focus),
                                }));
                            }

                            ui.layout(
                                Layout::vertical(match show_search {
                                    true => [2, 0],
                                    false => [1, 1],
                                }),
                                Some(0),
                                |ui| {
                                    use state::Section;
                                    if let Some(section) = focus {
                                        match Section::try_from(section).unwrap_or_default() {
                                            Section::Browser => {
                                                self.show_browser_context(frame, ui);
                                                self.show_browser_shortcuts(frame, ui);
                                            }
                                            Section::Issue => {
                                                self.show_issue_context(frame, ui);
                                                self.show_issue_shortcuts(frame, ui);
                                            }
                                            Section::Comment => {
                                                self.show_comment_context(frame, ui);
                                                self.show_comment_shortcuts(frame, ui);
                                            }
                                        }
                                    } else if show_search {
                                        self.show_browser_search(frame, ui);
                                    }
                                },
                            );
                        },
                    );

                    if !show_search {
                        if ui.has_input(|key| key == Key::Char('p')) {
                            ui.send_message(Message::Changed(Change::ShowPreview {
                                state: !self.state.preview.show,
                            }));
                        }
                        if ui.has_input(|key| key == Key::Char('?')) {
                            ui.send_message(Message::Changed(Change::Page {
                                page: state::Page::Help,
                            }));
                        }
                    }
                }
                state::Page::Help => {
                    let layout = Layout::vertical([
                        Constraint::Length(3),
                        Constraint::Fill(1),
                        Constraint::Length(1),
                        Constraint::Length(1),
                    ]);

                    ui.container(layout, &mut Some(1), |ui| {
                        self.show_help_text(frame, ui);
                        self.show_help_context(frame, ui);

                        ui.shortcuts(frame, &[("?", "close")], '∙', Alignment::Left);
                    });

                    if ui.has_input(|key| key == Key::Char('?')) {
                        ui.send_message(Message::Changed(Change::Page {
                            page: state::Page::Main,
                        }));
                    }
                }
            }

            if ui.has_input(|key| key == Key::Char('q')) {
                ui.send_message(Message::Quit);
            }
            if ui.has_input(|key| key == Key::Ctrl('c')) {
                ui.send_message(Message::Quit);
            }
        });

        Ok(())
    }
}

impl App {
    pub fn show_browser(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        let issues = self.issues.lock().unwrap();
        let issues = issues
            .iter()
            .filter(|patch| self.state.filter.matches(patch))
            .cloned()
            .collect::<Vec<_>>();
        let browser = &self.state.browser;
        let preview = &self.state.preview;
        let mut selected = browser.issues.selected();

        let header = [
            Column::new(" ● ", Constraint::Length(3)),
            Column::new("ID", Constraint::Length(8)),
            Column::new("Title", Constraint::Fill(5)),
            Column::new("Author", Constraint::Length(16)).hide_small(),
            Column::new("", Constraint::Length(16)).hide_medium(),
            Column::new("Labels", Constraint::Fill(1)).hide_medium(),
            Column::new("Assignees", Constraint::Fill(1)).hide_medium(),
            Column::new("Opened", Constraint::Length(16)).hide_small(),
        ];

        ui.layout(
            Layout::vertical([Constraint::Length(3), Constraint::Min(1)]),
            Some(1),
            |ui| {
                ui.column_bar(frame, header.to_vec(), Spacing::from(1), Some(Borders::Top));

                let table = ui.table(
                    frame,
                    &mut selected,
                    &issues,
                    header.to_vec(),
                    None,
                    Spacing::from(1),
                    Some(Borders::BottomSides),
                );
                if table.changed {
                    ui.send_message(Message::Changed(Change::Issue {
                        state: TableState::new(selected),
                    }));
                }
            },
        );

        if ui.has_input(|key| key == Key::Char('/')) {
            ui.send_message(Message::Changed(Change::ShowSearch {
                state: true,
                apply: false,
            }));
        }

        if let Some(issue) = selected.and_then(|s| issues.get(s)) {
            if ui.has_input(|key| key == Key::Enter) {
                ui.send_message(Message::Exit {
                    operation: Some(IssueOperation::Show { id: issue.id }),
                });
            }

            if ui.has_input(|key| key == Key::Char('e')) {
                ui.send_message(Message::Exit {
                    operation: Some(IssueOperation::Edit {
                        id: issue.id,
                        comment_id: preview.selected_comment().map(|c| c.id),
                        search: browser.search.read().text,
                    }),
                });
            }

            if ui.has_input(|key| key == Key::Char('s')) {
                ui.send_message(Message::Exit {
                    operation: Some(IssueOperation::Solve {
                        id: issue.id,
                        search: browser.search.read().text,
                    }),
                });
            }

            if ui.has_input(|key| key == Key::Char('l')) {
                ui.send_message(Message::Exit {
                    operation: Some(IssueOperation::Close {
                        id: issue.id,
                        search: browser.search.read().text,
                    }),
                });
            }

            if ui.has_input(|key| key == Key::Char('o')) {
                ui.send_message(Message::Exit {
                    operation: Some(IssueOperation::Reopen {
                        id: issue.id,
                        search: browser.search.read().text,
                    }),
                });
            }
        }
    }

    pub fn show_browser_search(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        let mut search = self.state.browser.search.clone();
        let (mut search_text, mut search_cursor) =
            (search.clone().read().text, search.clone().read().cursor);

        let text_edit = ui.text_edit_singleline(
            frame,
            &mut search_text,
            &mut search_cursor,
            Some("Search".to_string()),
            Some(Borders::Spacer { top: 0, left: 0 }),
        );

        if text_edit.changed {
            search.write(TextEditState {
                text: search_text,
                cursor: search_cursor,
            });
            ui.send_message(Message::Changed(Change::Search { state: search }));
        }

        if ui.has_input(|key| key == Key::Esc) {
            ui.send_message(Message::Changed(Change::ShowSearch {
                state: false,
                apply: false,
            }));
        }
        if ui.has_input(|key| key == Key::Enter) {
            ui.send_message(Message::Changed(Change::ShowSearch {
                state: false,
                apply: true,
            }));
        }
    }

    fn show_browser_context(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        use radicle::issue::{CloseReason, State};

        let context = {
            let issues = self.issues.lock().unwrap();
            let filter = &self.state.filter;
            let filtered = issues
                .iter()
                .filter(|issue| filter.matches(issue))
                .collect::<Vec<_>>();

            let browser = &self.state.browser;
            let search = browser.search.read().text;

            let mut open = 0;
            let mut other = 0;
            let mut solved = 0;
            for issue in &filtered {
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

            let filtered_counts = format!(" {}/{} ", filtered.len(), issues.len());
            if !self.state.filter.has_state() {
                [
                    Column::new(
                        Span::raw(" Search ".to_string()).cyan().dim().reversed(),
                        Constraint::Length(8),
                    ),
                    Column::new(
                        Span::raw(format!(" {search} "))
                            .into_left_aligned_line()
                            .style(ui.theme().bar_on_black_style)
                            .cyan()
                            .dim(),
                        Constraint::Fill(1),
                    ),
                    Column::new(
                        Span::raw(" ● ")
                            .into_right_aligned_line()
                            .style(ui.theme().bar_on_black_style)
                            .green()
                            .dim(),
                        Constraint::Length(3),
                    ),
                    Column::new(
                        Span::from(open.to_string())
                            .style(ui.theme().bar_on_black_style)
                            .into_right_aligned_line(),
                        Constraint::Length(open.to_string().chars().count() as u16),
                    ),
                    Column::new(
                        Span::raw(" ● ")
                            .style(ui.theme().bar_on_black_style)
                            .into_right_aligned_line()
                            .red()
                            .dim(),
                        Constraint::Length(3),
                    ),
                    Column::new(
                        Span::from(closed.to_string())
                            .style(ui.theme().bar_on_black_style)
                            .into_right_aligned_line(),
                        Constraint::Length(closed.to_string().chars().count() as u16),
                    ),
                    Column::new(
                        Span::from(" ")
                            .style(ui.theme().bar_on_black_style)
                            .into_right_aligned_line(),
                        Constraint::Length(1),
                    ),
                    Column::new(
                        Span::raw(filtered_counts.clone())
                            .into_right_aligned_line()
                            .cyan()
                            .dim()
                            .reversed(),
                        Constraint::Length(filtered_counts.chars().count() as u16),
                    ),
                ]
                .to_vec()
            } else {
                [
                    Column::new(
                        Span::raw(" Search ".to_string()).cyan().dim().reversed(),
                        Constraint::Length(8),
                    ),
                    Column::new(
                        Span::raw(format!(" {search} "))
                            .into_left_aligned_line()
                            .style(ui.theme().bar_on_black_style)
                            .cyan()
                            .dim(),
                        Constraint::Fill(1),
                    ),
                    Column::new(
                        Span::raw(filtered_counts.clone())
                            .into_right_aligned_line()
                            .cyan()
                            .dim()
                            .reversed(),
                        Constraint::Length(filtered_counts.chars().count() as u16),
                    ),
                ]
                .to_vec()
            }
        };

        ui.column_bar(frame, context, Spacing::from(0), Some(Borders::None));
    }

    pub fn show_browser_shortcuts(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        use radicle::issue::State;

        let issues = self.issues.lock().unwrap();
        let issues = issues
            .iter()
            .filter(|issue| self.state.filter.matches(issue))
            .collect::<Vec<_>>();

        let mut shortcuts = vec![("/", "search"), ("enter", "show"), ("e", "edit")];
        if let Some(issue) = self.state.browser.selected().and_then(|i| issues.get(i)) {
            let actions = match issue.state {
                State::Open => vec![("s", "solve"), ("l", "close")],
                State::Closed { .. } => vec![("o", "re-open")],
            };
            shortcuts.extend_from_slice(&actions);
        }

        let global_shortcuts = vec![("p", "toggle preview"), ("?", "help")];

        ui.layout(
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(30)]),
            None,
            |ui| {
                ui.shortcuts(frame, &shortcuts, '∙', Alignment::Left);
                ui.shortcuts(frame, &global_shortcuts, '∙', Alignment::Right);
            },
        );
    }

    pub fn show_issue(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        #[derive(Clone)]
        struct Property<'a>(Span<'a>, Text<'a>);

        impl<'a> ToRow<3> for Property<'a> {
            fn to_row(&self) -> [ratatui::widgets::Cell<'_>; 3] {
                ["".into(), self.0.clone().into(), self.1.clone().into()]
            }
        }

        let issues = self.issues.lock().unwrap();
        let issues = issues
            .iter()
            .filter(|issue| self.state.filter.matches(issue))
            .collect::<Vec<_>>();
        let issue = self.state.browser.selected().and_then(|i| issues.get(i));
        let properties = issue
            .map(|issue| {
                use radicle::issue;

                let author: Text<'_> = match &issue.author.alias {
                    Some(alias) => {
                        if issue.author.you {
                            Line::from(
                                [
                                    span::alias(alias.as_ref()),
                                    Span::raw(" "),
                                    span::alias("(you)").dim().italic(),
                                ]
                                .to_vec(),
                            )
                            .into()
                        } else {
                            Line::from(
                                [
                                    span::alias(alias.as_ref()),
                                    Span::raw(" "),
                                    span::alias(&format!(
                                        "({})",
                                        issue.author.human_nid.clone().unwrap_or_default()
                                    ))
                                    .dim()
                                    .italic(),
                                ]
                                .to_vec(),
                            )
                            .into()
                        }
                    }
                    None => match &issue.author.human_nid {
                        Some(nid) => span::alias(nid).dim().into(),
                        None => span::blank().into(),
                    },
                };

                let status = match issue.state {
                    issue::State::Open => Text::from("open").green(),
                    issue::State::Closed { reason } => match reason {
                        issue::CloseReason::Solved => Line::from(
                            [
                                Span::from("closed").red(),
                                Span::raw(" "),
                                Span::from("(solved)").red().italic().dim(),
                            ]
                            .to_vec(),
                        )
                        .into(),
                        issue::CloseReason::Other => Text::from("closed").red(),
                    },
                };

                vec![
                    Property(Span::from("Title"), Text::from(issue.title.clone()).bold()),
                    Property(Span::from("Issue"), Text::from(issue.id.to_string()).cyan()),
                    Property(Span::from("Author"), author.magenta()),
                    Property(
                        Span::from("Labels"),
                        Text::from(format::labels(&issue.labels)).blue(),
                    ),
                    Property(Span::from("Status"), status),
                ]
            })
            .unwrap_or_default();

        let browser = &self.state.browser;
        let search = browser.search.read();

        let preview = &self.state.preview;
        let comment = preview.selected_comment();
        let root = preview.root_comments();
        let mut opened = Some(preview.opened_comments());
        let mut selected = Some(preview.selected_comment_ids());

        ui.layout(
            Layout::vertical([Constraint::Length(7), Constraint::Fill(1)]),
            Some(1),
            |ui| {
                ui.table(
                    frame,
                    &mut None,
                    &properties,
                    vec![
                        Column::new("", Constraint::Length(1)),
                        Column::new("", Constraint::Length(12)),
                        Column::new("", Constraint::Fill(1)),
                    ],
                    None,
                    Spacing::from(0),
                    Some(Borders::Top),
                );
                let comments = ui.tree(
                    frame,
                    &root,
                    &mut opened,
                    &mut selected,
                    Some(Borders::BottomSides),
                );
                if comments.changed {
                    let mut state = tui_tree_widget::TreeState::default();
                    if let Some(opened) = opened {
                        for open in opened {
                            state.open(open);
                        }
                    }
                    if let Some(selected) = selected {
                        state.select(selected);
                    }

                    ui.send_message(Message::Changed(Change::Comment {
                        state: TreeState { internal: state },
                    }));
                }

                if let Some(issue) = issue {
                    if ui.has_input(|key| key == Key::Char('c')) {
                        ui.send_message(Message::Exit {
                            operation: Some(IssueOperation::Comment {
                                id: issue.id,
                                reply_to: comment.map(|c| c.id),
                                search: search.text.clone(),
                            }),
                        });
                    }

                    if ui.has_input(|key| key == Key::Char('e')) {
                        ui.send_message(Message::Exit {
                            operation: Some(IssueOperation::Edit {
                                id: issue.id,
                                comment_id: comment.map(|c| c.id),
                                search: search.text,
                            }),
                        });
                    }
                }
            },
        );
    }

    pub fn show_issue_context(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        ui.column_bar(
            frame,
            [
                Column::new(
                    Span::raw(" Comment ".to_string()).cyan().dim().reversed(),
                    Constraint::Length(9),
                ),
                Column::new(
                    Span::raw(" ".to_string())
                        .into_left_aligned_line()
                        .style(ui.theme().bar_on_black_style),
                    Constraint::Fill(1),
                ),
            ]
            .to_vec(),
            Spacing::from(0),
            Some(Borders::None),
        );
    }

    pub fn show_issue_shortcuts(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        let shortcuts = vec![("e", "edit"), ("c", "reply")];
        let global_shortcuts = vec![("p", "toggle preview"), ("?", "help")];

        ui.layout(
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(30)]),
            None,
            |ui| {
                ui.shortcuts(frame, &shortcuts, '∙', Alignment::Left);
                ui.shortcuts(frame, &global_shortcuts, '∙', Alignment::Right);
            },
        );
    }

    pub fn show_comment(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        let (text, footer, mut cursor) = {
            let comment = self.state.preview.selected_comment();
            let body: String = comment
                .map(|comment| comment.body.clone())
                .unwrap_or_default();
            let reactions = comment
                .map(|comment| {
                    let reactions = comment.accumulated_reactions().iter().fold(
                        String::new(),
                        |all, (r, acc)| {
                            if *acc > 1_usize {
                                [all, format!("{r}{acc} ")].concat()
                            } else {
                                [all, format!("{r} ")].concat()
                            }
                        },
                    );
                    reactions
                })
                .unwrap_or_default();

            (body, reactions, self.state.preview.comment.clone().cursor())
        };
        let comment =
            ui.text_view_with_footer(frame, text, footer, &mut cursor, Some(Borders::All));
        if comment.changed {
            ui.send_message(Message::Changed(Change::CommentBody {
                state: TextViewState::new(cursor),
            }))
        }
    }

    pub fn show_comment_context(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        ui.column_bar(
            frame,
            [
                Column::new(
                    Span::raw(" Comment ".to_string()).cyan().dim().reversed(),
                    Constraint::Length(9),
                ),
                Column::new(
                    Span::raw(" ".to_string())
                        .into_left_aligned_line()
                        .style(ui.theme().bar_on_black_style),
                    Constraint::Fill(1),
                ),
            ]
            .to_vec(),
            Spacing::from(0),
            Some(Borders::None),
        );
    }

    pub fn show_comment_shortcuts(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        let shortcuts = vec![("e", "edit"), ("c", "reply")];
        let global_shortcuts = vec![("p", "toggle preview"), ("?", "help")];

        ui.layout(
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(30)]),
            None,
            |ui| {
                ui.shortcuts(frame, &shortcuts, '∙', Alignment::Left);
                ui.shortcuts(frame, &global_shortcuts, '∙', Alignment::Right);
            },
        );
    }

    fn show_help_text(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        ui.column_bar(
            frame,
            [Column::new(Span::raw(" Help ").bold(), Constraint::Fill(1))].to_vec(),
            Spacing::from(0),
            Some(Borders::Top),
        );

        let mut cursor = self.state.help.cursor();
        let text_view = ui.text_view(
            frame,
            HELP.to_string(),
            &mut cursor,
            Some(Borders::BottomSides),
        );
        if text_view.changed {
            ui.send_message(Message::Changed(Change::Help {
                state: TextViewState::new(cursor),
            }))
        }
    }

    fn show_help_context(&self, frame: &mut Frame, ui: &mut Ui<Message>) {
        ui.column_bar(
            frame,
            [
                Column::new(
                    Span::raw(" ".to_string())
                        .into_left_aligned_line()
                        .style(ui.theme().bar_on_black_style),
                    Constraint::Fill(1),
                ),
                Column::new(
                    Span::raw(" ")
                        .into_right_aligned_line()
                        .cyan()
                        .dim()
                        .reversed(),
                    Constraint::Length(6),
                ),
            ]
            .to_vec(),
            Spacing::from(0),
            Some(Borders::None),
        );
    }
}
