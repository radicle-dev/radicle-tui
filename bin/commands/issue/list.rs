#[path = "list/ui.rs"]
mod ui;

use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use anyhow::{bail, Result};

use ratatui::layout::Constraint;
use ratatui::style::Stylize;
use ratatui::text::Text;
use ratatui::Viewport;

use radicle::cob::thread::CommentId;
use radicle::git::Oid;
use radicle::issue::IssueId;
use radicle::storage::git::Repository;
use radicle::Profile;

use radicle_tui as tui;

use tui::event::{Event, Key};
use tui::store;
use tui::task::EmptyProcessors;
use tui::ui::rm::widget::container::{
    Container, ContainerProps, Footer, FooterProps, Header, HeaderProps, SectionGroup,
    SectionGroupProps, SplitContainer, SplitContainerFocus, SplitContainerProps,
};
use tui::ui::rm::widget::list::{Tree, TreeProps};
use tui::ui::rm::widget::text::{TextView, TextViewProps, TextViewState};
use tui::ui::rm::widget::window::{
    Page, PageProps, Shortcuts, ShortcutsProps, Window, WindowProps,
};
use tui::ui::rm::widget::{PredefinedLayout, ToWidget, Widget};
use tui::ui::theme::Theme;
use tui::ui::Column;
use tui::ui::{span, BufferedValue};
use tui::{BoxedAny, Channel, Exit, PageStack};

use crate::cob::issue;
use crate::settings::{self, ThemeBundle, ThemeMode};
use crate::ui::items::issue::{Issue, IssueFilter};
use crate::ui::items::CommentItem;
use crate::ui::rm::{BrowserState, IssueDetails, IssueDetailsProps};
use crate::ui::TerminalInfo;

use self::ui::{Browser, BrowserProps};

use super::common::IssueOperation;

type Selection = tui::Selection<IssueOperation>;

pub(crate) struct Context {
    pub(crate) profile: Profile,
    pub(crate) repository: Repository,
    pub(crate) filter: issue::Filter,
    pub(crate) search: Option<String>,
    pub(crate) issue: Option<IssueId>,
    pub(crate) comment: Option<CommentId>,
}

pub struct App {
    context: Context,
    terminal_info: TerminalInfo,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum AppPage {
    Browser,
    Help,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub enum Section {
    #[default]
    Browser,
    Details,
    Comment,
}

impl TryFrom<usize> for Section {
    type Error = anyhow::Error;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Section::Browser),
            1 => Ok(Section::Details),
            2 => Ok(Section::Comment),
            _ => bail!("Unknown section index: {}", value),
        }
    }
}

impl From<Section> for usize {
    fn from(section: Section) -> Self {
        match section {
            Section::Browser => 0,
            Section::Details => 1,
            Section::Comment => 2,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PreviewState {
    /// If preview is visible.
    show: bool,
    /// Currently selected issue item.
    issue: Option<Issue>,
    /// Tree selection per issue.
    selected_comments: HashMap<IssueId, Vec<CommentId>>,
    /// State of currently selected comment
    comment: TextViewState,
}

impl PreviewState {
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

#[derive(Clone, Debug)]
pub struct HelpState {
    text: TextViewState,
}

#[derive(Clone, Debug)]
pub struct State {
    pages: PageStack<AppPage>,
    browser: BrowserState<Issue, IssueFilter>,
    preview: PreviewState,
    section: Option<Section>,
    help: HelpState,
    theme: Theme,
}

impl TryFrom<(&Context, &TerminalInfo)> for State {
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
        let theme = match settings.theme.mode() {
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

        let browser = BrowserState::build(issues, context.issue, filter, search);
        let preview = PreviewState {
            show: true,
            issue: browser.selected_item().cloned(),
            selected_comments,
            comment: TextViewState::default(),
        };

        let section = if context.comment.is_some() {
            Some(Section::Details)
        } else {
            Some(Section::Browser)
        };

        Ok(Self {
            pages: PageStack::new(vec![AppPage::Browser]),
            browser,
            preview,
            section,
            help: HelpState {
                text: TextViewState::default().content(help_text()),
            },
            theme,
        })
    }
}

#[derive(Clone, Debug)]
pub enum RequestedIssueOperation {
    Edit,
    EditComment,
    Show,
    Reply,
    Solve,
    Close,
    Reopen,
}

#[derive(Clone, Debug)]
pub enum Message {
    Quit,
    Exit {
        operation: Option<RequestedIssueOperation>,
    },
    SelectIssue {
        selected: Option<usize>,
    },
    OpenSearch,
    UpdateSearch {
        value: String,
    },
    ApplySearch,
    CloseSearch,
    TogglePreview,
    FocusSection {
        section: Option<Section>,
    },
    SelectComment {
        selected: Option<Vec<CommentId>>,
    },
    ScrollComment {
        state: TextViewState,
    },
    OpenHelp,
    LeavePage,
    ScrollHelp {
        state: TextViewState,
    },
}

impl store::Update<Message> for State {
    type Return = Selection;

    fn update(&mut self, message: Message) -> Option<Exit<Selection>> {
        match message {
            Message::Quit => Some(Exit { value: None }),
            Message::Exit { operation } => {
                let issue = self.browser.selected_item();
                let comment = self.preview.selected_comment();
                let operation = match operation {
                    Some(RequestedIssueOperation::Show) => {
                        issue.map(|issue| IssueOperation::Show { id: issue.id })
                    }
                    Some(RequestedIssueOperation::Edit) => {
                        issue.map(|issue| IssueOperation::Edit {
                            id: issue.id,
                            comment_id: None,
                            search: self.browser.read_search(),
                        })
                    }
                    Some(RequestedIssueOperation::EditComment) => {
                        issue.map(|issue| IssueOperation::Edit {
                            id: issue.id,
                            comment_id: comment.map(|c| c.id),
                            search: self.browser.read_search(),
                        })
                    }
                    Some(RequestedIssueOperation::Solve) => {
                        issue.map(|issue| IssueOperation::Solve { id: issue.id })
                    }
                    Some(RequestedIssueOperation::Close) => {
                        issue.map(|issue| IssueOperation::Close { id: issue.id })
                    }
                    Some(RequestedIssueOperation::Reopen) => {
                        issue.map(|issue| IssueOperation::Reopen { id: issue.id })
                    }
                    Some(RequestedIssueOperation::Reply) => {
                        issue.map(|issue| IssueOperation::Comment {
                            id: issue.id,
                            reply_to: comment.map(|c| c.id),
                            search: self.browser.read_search(),
                        })
                    }
                    _ => None,
                };
                Some(Exit {
                    value: Some(Selection {
                        operation,
                        args: vec![],
                    }),
                })
            }
            Message::SelectIssue { selected } => {
                self.browser.select_item(selected);
                self.preview.issue = self.browser.selected_item().cloned();
                self.preview.comment.reset_cursor();
                None
            }
            Message::TogglePreview => {
                self.preview.show = !self.preview.show;
                self.section = Some(Section::Browser);
                None
            }
            Message::FocusSection { section } => {
                self.section = section;
                None
            }
            Message::SelectComment { selected } => {
                if let Some(item) = &self.preview.issue {
                    self.preview
                        .selected_comments
                        .insert(item.id, selected.unwrap_or(vec![]));
                }
                self.preview.comment.reset_cursor();
                None
            }
            Message::ScrollComment { state } => {
                self.preview.comment = state;
                None
            }
            Message::OpenSearch => {
                self.browser.show_search();
                None
            }
            Message::UpdateSearch { value } => {
                self.browser.update_search(value);
                self.preview.issue = self.browser.select_first_item().cloned();
                None
            }
            Message::ApplySearch => {
                self.browser.hide_search();
                self.browser.apply_search();
                None
            }
            Message::CloseSearch => {
                self.browser.hide_search();
                self.browser.reset_search();

                self.preview.issue = self.browser.selected_item().cloned();
                self.preview.comment.reset_cursor();
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
            Message::ScrollHelp { state } => {
                self.help.text = state;
                None
            }
        }
    }
}

impl App {
    pub fn new(context: Context, terminal_info: TerminalInfo) -> Self {
        Self {
            context,
            terminal_info,
        }
    }

    pub async fn run(&self) -> Result<Option<Selection>> {
        let channel = Channel::default();
        let state = State::try_from((&self.context, &self.terminal_info))?;
        let tx = channel.tx.clone();

        let window = Window::default()
            .page(AppPage::Browser, browser_page(&channel))
            .page(AppPage::Help, help_page(&channel))
            .to_widget(tx.clone())
            .on_update(|state| {
                WindowProps::default()
                    .current_page(state.pages.peek().unwrap_or(&AppPage::Browser).clone())
                    .to_boxed_any()
                    .into()
            });

        tui::rm(
            state,
            window,
            Viewport::Inline(20),
            channel,
            EmptyProcessors::new(),
        )
        .await
    }

    pub fn context(&self) -> &Context {
        &self.context
    }
}

fn browser_page(channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    let shortcuts = Shortcuts::default()
        .to_widget(tx.clone())
        .on_update(|state: &State| {
            let shortcuts = if state.browser.is_search_shown() {
                vec![("esc", "cancel"), ("enter", "apply")]
            } else {
                match state.section {
                    Some(Section::Browser) => {
                        let mut shortcuts =
                            [("/", "search"), ("enter", "show"), ("e", "edit")].to_vec();
                        if let Some(issue) = state.browser.selected_item() {
                            use radicle::issue::State;
                            let actions = match issue.state {
                                State::Open => [("s", "solve"), ("l", "close")].to_vec(),
                                State::Closed { .. } => [("o", "re-open")].to_vec(),
                            };
                            shortcuts = [shortcuts, actions.to_vec()].concat();
                        }
                        shortcuts
                    }
                    _ => [("e", "edit"), ("c", "reply")].to_vec(),
                }
            };
            let global_shortcuts = vec![("p", "toggle preview"), ("?", "help")];

            ShortcutsProps::default()
                .shortcuts(&shortcuts)
                .global_shortcuts(&global_shortcuts)
                .shortcuts_keys_style(state.theme.shortcuts_keys_style)
                .shortcuts_action_style(state.theme.shortcuts_action_style)
                .to_boxed_any()
                .into()
        });

    Page::default()
        .content(
            SectionGroup::default()
                .section(browser(channel))
                .section(issue(channel))
                .section(comment(channel))
                .to_widget(tx.clone())
                .on_event(|_, vs, _| {
                    Some(Message::FocusSection {
                        section: vs.and_then(|vs| {
                            vs.unwrap_section_group()
                                .and_then(|sgs| sgs.focus)
                                .map(|s| s.try_into().unwrap_or_default())
                        }),
                    })
                })
                .on_update(|state: &State| {
                    SectionGroupProps::default()
                        .handle_keys(state.preview.show && !state.browser.is_search_shown())
                        .layout(PredefinedLayout::Expandable3 {
                            left_only: !state.preview.show,
                        })
                        .focus(state.section.as_ref().map(|s| s.clone().into()))
                        .to_boxed_any()
                        .into()
                }),
        )
        .shortcuts(shortcuts)
        .to_widget(tx.clone())
        .on_event(|event, _, props| {
            let default = PageProps::default();
            let props = props
                .and_then(|props| props.inner_ref::<PageProps>())
                .unwrap_or(&default);
            if props.handle_keys {
                if let Event::Key(key) = event {
                    match key {
                        Key::Char('q') | Key::Ctrl('c') => Some(Message::Quit),
                        Key::Char('p') => Some(Message::TogglePreview),
                        Key::Char('?') => Some(Message::OpenHelp),
                        _ => None,
                    }
                } else {
                    None
                }
            } else {
                None
            }
        })
        .on_update(|state: &State| {
            PageProps::default()
                .handle_keys(!state.browser.is_search_shown())
                .to_boxed_any()
                .into()
        })
}

fn browser(channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    Browser::new(tx.clone())
        .to_widget(tx.clone())
        .on_update(|state| BrowserProps::from(state).to_boxed_any().into())
        .on_event(|event, _, props| {
            let default = BrowserProps::default();
            let props = props
                .and_then(|props| props.inner_ref::<BrowserProps>())
                .unwrap_or(&default);
            if !props.show_search() {
                if let Event::Key(key) = event {
                    match key {
                        Key::Enter => Some(Message::Exit {
                            operation: Some(RequestedIssueOperation::Show),
                        }),
                        Key::Char('e') => Some(Message::Exit {
                            operation: Some(RequestedIssueOperation::Edit),
                        }),
                        Key::Char('s') => Some(Message::Exit {
                            operation: Some(RequestedIssueOperation::Solve),
                        }),
                        Key::Char('l') => Some(Message::Exit {
                            operation: Some(RequestedIssueOperation::Close),
                        }),
                        Key::Char('o') => Some(Message::Exit {
                            operation: Some(RequestedIssueOperation::Reopen),
                        }),
                        _ => None,
                    }
                } else {
                    None
                }
            } else {
                None
            }
        })
}

fn issue(channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    SplitContainer::default()
        .top(issue_details(channel))
        .bottom(comment_tree(channel))
        .to_widget(tx.clone())
        .on_update(|state| {
            SplitContainerProps::default()
                .heights([Constraint::Length(5), Constraint::Min(1)])
                .border_style(state.theme.border_style)
                .focus_border_style(state.theme.focus_border_style)
                .split_focus(SplitContainerFocus::Bottom)
                .to_boxed_any()
                .into()
        })
}

fn issue_details(channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    IssueDetails::default()
        .to_widget(tx.clone())
        .on_update(|state: &State| {
            IssueDetailsProps::default()
                .issue(state.preview.issue.clone())
                .dim(state.theme.dim_no_focus)
                .to_boxed_any()
                .into()
        })
}

fn comment_tree(channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    Tree::<State, Message, CommentItem, String>::default()
        .to_widget(tx.clone())
        .on_update(|state| {
            let root = &state.preview.root_comments();
            let opened = &state.preview.opened_comments();
            let selected = &state.preview.selected_comment_ids();

            TreeProps::<CommentItem, String>::default()
                .items(root.to_vec())
                .selected(Some(selected))
                .opened(Some(opened.clone()))
                .dim(state.theme.dim_no_focus)
                .to_boxed_any()
                .into()
        })
        .on_event(|event, s, _| match event {
            Event::Key(Key::Char('c')) => Some(Message::Exit {
                operation: Some(RequestedIssueOperation::Reply),
            }),
            Event::Key(Key::Char('e')) => Some(Message::Exit {
                operation: Some(RequestedIssueOperation::EditComment),
            }),
            _ => Some(Message::SelectComment {
                selected: s.and_then(|s| {
                    s.unwrap_tree()
                        .map(|tree| tree.iter().map(|id| Oid::from_str(id).unwrap()).collect())
                }),
            }),
        })
}

fn comment(channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    Container::default()
        .content(
            TextView::default()
                .to_widget(tx.clone())
                .on_event(|_, vs, _| {
                    let state = vs.and_then(|p| p.unwrap_textview()).unwrap_or_default();
                    Some(Message::ScrollComment { state })
                })
                .on_update(|state: &State| {
                    let comment = state.preview.selected_comment();
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

                    TextViewProps::default()
                        .state(Some(state.preview.comment.clone().content(body)))
                        .footer(Some(reactions))
                        .show_scroll_progress(true)
                        .dim(state.theme.dim_no_focus)
                        .to_boxed_any()
                        .into()
                }),
        )
        .to_widget(tx.clone())
        .on_update(|state| {
            ContainerProps::default()
                .border_style(state.theme.border_style)
                .focus_border_style(state.theme.focus_border_style)
                .to_boxed_any()
                .into()
        })
        .on_event(|event, _, _| match event {
            Event::Key(Key::Char('c')) => Some(Message::Exit {
                operation: Some(RequestedIssueOperation::Reply),
            }),
            Event::Key(Key::Char('e')) => Some(Message::Exit {
                operation: Some(RequestedIssueOperation::EditComment),
            }),
            _ => None,
        })
}

fn help_page(channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    let content = Container::default()
        .header(Header::default().to_widget(tx.clone()).on_update(|_| {
            HeaderProps::default()
                .columns([Column::new(" Help ", Constraint::Fill(1))].to_vec())
                .to_boxed_any()
                .into()
        }))
        .content(
            TextView::default()
                .to_widget(tx.clone())
                .on_event(|_, view_state, _| {
                    view_state
                        .and_then(|tv| tv.unwrap_textview())
                        .map(|tvs| Message::ScrollHelp { state: tvs })
                })
                .on_update(|state: &State| {
                    TextViewProps::default()
                        .state(Some(state.help.text.clone()))
                        .dim(state.theme.dim_no_focus)
                        .to_boxed_any()
                        .into()
                }),
        )
        .footer(
            Footer::default()
                .to_widget(tx.clone())
                .on_update(|state: &State| {
                    FooterProps::default()
                        .columns(
                            [
                                Column::new(Text::raw(""), Constraint::Fill(1)),
                                Column::new(
                                    span::default(&format!("{}%", state.help.text.scroll)).dim(),
                                    Constraint::Min(4),
                                ),
                            ]
                            .to_vec(),
                        )
                        .to_boxed_any()
                        .into()
                }),
        )
        .to_widget(tx.clone())
        .on_update(|state| {
            ContainerProps::default()
                .border_style(state.theme.border_style)
                .focus_border_style(state.theme.focus_border_style)
                .to_boxed_any()
                .into()
        });

    let shortcuts = Shortcuts::default().to_widget(tx.clone()).on_update(|_| {
        ShortcutsProps::default()
            .shortcuts(&[("?", "close")])
            .to_boxed_any()
            .into()
    });

    Page::default()
        .content(content)
        .shortcuts(shortcuts)
        .to_widget(tx.clone())
        .on_event(|event, _, _| match event {
            Event::Key(Key::Char('q')) | Event::Key(Key::Ctrl('c')) => Some(Message::Quit),
            Event::Key(Key::Char('?')) => Some(Message::LeavePage),
            _ => None,
        })
        .on_update(|_| PageProps::default().handle_keys(true).to_boxed_any().into())
}

fn help_text() -> String {
    r#"# Generic keybindings

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

`Enter`:    Show issue
`e`:        Edit issue
`c`:        Reply to comment
`p`:        Toggle issue preview
`/`:        Search
`?`:        Show help

# Searching

Pattern:    is:<state> | is:authored | is:assigned | authors:[<did>, ...] | assignees:[<did>, ...] | <search>
Example:    is:solved is:authored alias"#
        .into()
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

pub mod v2 {
    use std::collections::{HashMap, HashSet};
    use std::str::FromStr;
    use std::sync::{Arc, Mutex};

    use anyhow::{bail, Result};

    use radicle_tui::ui::im::widget::TreeState;
    use radicle_tui::ui::ToRow;
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
    use tui::ui::im;
    use tui::ui::im::widget::{ContainerState, TableState, TextEditState, TextViewState, Window};
    use tui::ui::im::{Borders, Show};
    use tui::ui::Column;
    use tui::ui::{span, BufferedValue, Spacing};
    use tui::{Channel, Exit};

    use crate::cob::issue;
    use crate::commands::tui_issue::list::v2::state::{Browser, Preview, Section};
    use crate::settings::{self, ThemeBundle, ThemeMode};
    use crate::ui::items::filter::Filter;
    use crate::ui::items::issue::{Issue, IssueFilter};
    use crate::ui::items::HasId;
    use crate::ui::{format, TerminalInfo};

    use crate::tui_issue::common::IssueOperation;

    type Selection = tui::Selection<IssueOperation>;

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
        pub filter: issue::Filter,
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
        use crate::tui_issue::list::append_opened;
        use crate::ui::items::CommentItem;

        use super::*;

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
                        .and_then(|comment_id| {
                            item.comments.iter().find(|item| item.id == comment_id)
                        })
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

            let browser = Browser {
                issues: TableState::new(
                    context
                        .issue
                        .map(|id| {
                            issues
                                .iter()
                                .filter(|item| filter.matches(item))
                                .position(|item| item.id() == id)
                        })
                        .unwrap_or(issues.first().map(|_| 0)),
                ),
                search: BufferedValue::new(TextEditState {
                    text: search.read().clone(),
                    cursor: search.read().len(),
                }),
                show_search: false,
            };

            let preview = Preview {
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
                state::Section::Comment
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
                            self.state.sections =
                                ContainerState::new(self.state.sections.len(), None);
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
                        self.state.sections =
                            ContainerState::new(if state { 3 } else { 1 }, Some(0));
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
                        log::info!("Change::Comments: {state:?}");
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
        fn show(&self, ctx: &im::Context<Message>, frame: &mut Frame) -> Result<()> {
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
                                    im::Layout::Expandable3 {
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
        pub fn show_browser(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
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
                        operation: Some(IssueOperation::Solve { id: issue.id }),
                    });
                }

                if ui.has_input(|key| key == Key::Char('l')) {
                    ui.send_message(Message::Exit {
                        operation: Some(IssueOperation::Close { id: issue.id }),
                    });
                }

                if ui.has_input(|key| key == Key::Char('o')) {
                    ui.send_message(Message::Exit {
                        operation: Some(IssueOperation::Reopen { id: issue.id }),
                    });
                }
            }
        }

        pub fn show_browser_search(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
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

        fn show_browser_context(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
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
                let mut columns = vec![
                    Column::new(
                        Span::raw(" Issue ".to_string()).cyan().dim().reversed(),
                        Constraint::Length(7),
                    ),
                    Column::new(
                        Span::raw(format!(" {search} "))
                            .into_left_aligned_line()
                            .style(ui.theme().bar_on_black_style)
                            .cyan()
                            .dim(),
                        Constraint::Fill(1),
                    ),
                ];

                if filter.state().is_none() {
                    columns.extend_from_slice(&[
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
                    ]);
                }

                columns.extend_from_slice(&[Column::new(
                    Span::raw(filtered_counts.clone())
                        .into_right_aligned_line()
                        .cyan()
                        .dim()
                        .reversed(),
                    Constraint::Length(filtered_counts.chars().count() as u16),
                )]);
                columns
            };

            ui.column_bar(frame, context, Spacing::from(0), Some(Borders::None));
        }

        pub fn show_browser_shortcuts(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
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

        pub fn show_issue(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
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

        pub fn show_issue_context(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
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

        pub fn show_issue_shortcuts(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
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

        pub fn show_comment(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
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

        pub fn show_comment_context(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
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

        pub fn show_comment_shortcuts(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
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

        fn show_help_text(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
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

        fn show_help_context(&self, frame: &mut Frame, ui: &mut im::Ui<Message>) {
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
}
