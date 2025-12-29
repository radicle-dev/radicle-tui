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
use crate::ui::items::{CommentItem, IssueItem, IssueItemFilter};
use crate::ui::rm::{BrowserState, IssueDetails, IssueDetailsProps};
use crate::ui::TerminalInfo;

use self::ui::{Browser, BrowserProps};

use super::common::IssueOperation;

type Selection = tui::Selection<IssueOperation>;

pub struct Context {
    pub profile: Profile,
    pub repository: Repository,
    pub filter: issue::Filter,
    pub search: Option<String>,
    pub issue: Option<IssueId>,
    pub comment: Option<CommentId>,
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
    issue: Option<IssueItem>,
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
    browser: BrowserState<IssueItem, IssueItemFilter>,
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
        let filter = IssueItemFilter::from_str(&search.read()).unwrap_or_default();

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
            .flat_map(|issue| IssueItem::new(&context.profile, issue).ok())
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
