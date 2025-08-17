#[path = "list/imui.rs"]
mod imui;
#[path = "list/rmui.rs"]
mod rmui;

use std::str::FromStr;

use anyhow::Result;

use termion::event::Key;

use ratatui::layout::Constraint;
use ratatui::style::Stylize;
use ratatui::text::Text;
use ratatui::Viewport;

use radicle::patch::PatchId;
use radicle::storage::git::Repository;
use radicle::Profile;

use radicle_tui as tui;

use tui::store;
use tui::task::EmptyProcessors;
use tui::ui::rm::widget::container::{Container, Footer, FooterProps, Header, HeaderProps};
use tui::ui::rm::widget::text::{TextView, TextViewProps, TextViewState};
use tui::ui::rm::widget::window::{
    Page, PageProps, Shortcuts, ShortcutsProps, Window, WindowProps,
};
use tui::ui::rm::widget::{ToWidget, Widget};
use tui::ui::Column;
use tui::ui::{span, BufferedValue};
use tui::{BoxedAny, Channel, Exit, PageStack};

use self::rmui::{Browser, BrowserProps};
use super::common::{Mode, PatchOperation};

use crate::cob::patch;
use crate::ui::items::{PatchItem, PatchItemFilter};
use crate::ui::rm::BrowserState;

type Selection = tui::Selection<PatchId>;

pub struct Context {
    pub profile: Profile,
    pub repository: Repository,
    pub mode: Mode,
    pub filter: patch::Filter,
}

pub struct App {
    context: Context,
    im: bool,
}

impl App {
    pub fn new(context: Context, im: bool) -> Self {
        Self { context, im }
    }

    pub async fn run(&self) -> Result<Option<Selection>> {
        let viewport = Viewport::Inline(20);

        if self.im {
            let channel = Channel::default();
            let state = imui::App::try_from(&self.context)?;

            tui::im(state, viewport, channel, EmptyProcessors::new()).await
        } else {
            let channel = Channel::default();
            let tx = channel.tx.clone();
            let state = State::try_from(&self.context)?;
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

            tui::rm(state, window, viewport, channel, EmptyProcessors::new()).await
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum AppPage {
    Browse,
    Help,
}

#[derive(Clone, Debug)]
pub struct HelpState {
    text: TextViewState,
}

#[derive(Clone, Debug)]
pub struct State {
    mode: Mode,
    pages: PageStack<AppPage>,
    browser: BrowserState<PatchItem, PatchItemFilter>,
    help: HelpState,
}

impl TryFrom<&Context> for State {
    type Error = anyhow::Error;

    fn try_from(context: &Context) -> Result<Self, Self::Error> {
        let patches = patch::all(&context.profile, &context.repository)?;
        let search = BufferedValue::new(context.filter.to_string());
        let filter = PatchItemFilter::from_str(&context.filter.to_string()).unwrap_or_default();

        // Convert into UI items
        let mut items: Vec<_> = patches
            .into_iter()
            .flat_map(|patch| {
                PatchItem::new(&context.profile, &context.repository, patch.clone()).ok()
            })
            .collect();

        items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(Self {
            mode: context.mode.clone(),
            pages: PageStack::new(vec![AppPage::Browse]),
            browser: BrowserState::build(items.clone(), filter, search),
            help: HelpState {
                text: TextViewState::default().content(help_text()),
            },
        })
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    Quit,
    Exit { operation: Option<PatchOperation> },
    ExitFromMode,
    SelectPatch { selected: Option<usize> },
    OpenSearch,
    UpdateSearch { value: String },
    ApplySearch,
    CloseSearch,
    OpenHelp,
    LeavePage,
    ScrollHelp { state: TextViewState },
}

impl store::Update<Message> for State {
    type Return = Selection;

    fn update(&mut self, message: Message) -> Option<Exit<Selection>> {
        match message {
            Message::Quit => Some(Exit { value: None }),
            Message::Exit { operation } => self.browser.selected_item().map(|issue| Exit {
                value: Some(Selection {
                    operation: operation.map(|op| op.to_string()),
                    ids: vec![issue.id],
                    args: vec![],
                }),
            }),
            Message::ExitFromMode => {
                let operation = match self.mode {
                    Mode::Operation => Some(PatchOperation::Show.to_string()),
                    Mode::Id => None,
                };

                self.browser.selected_item().map(|issue| Exit {
                    value: Some(Selection {
                        operation,
                        ids: vec![issue.id],
                        args: vec![],
                    }),
                })
            }
            Message::SelectPatch { selected } => {
                self.browser.select_item(selected);
                None
            }
            Message::OpenSearch => {
                self.browser.show_search();
                None
            }
            Message::UpdateSearch { value } => {
                self.browser.update_search(value);
                self.browser.select_first_item();
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

fn browser_page(_state: &State, channel: &Channel<Message>) -> Widget<State, Message> {
    let tx = channel.tx.clone();

    let content = Browser::new(tx.clone())
        .to_widget(tx.clone())
        .on_update(|state| BrowserProps::from(state).to_boxed_any().into());

    let shortcuts = Shortcuts::default()
        .to_widget(tx.clone())
        .on_update(|state: &State| {
            let shortcuts = if state.browser.is_search_shown() {
                vec![("esc", "cancel"), ("enter", "apply")]
            } else {
                match state.mode {
                    Mode::Id => vec![("enter", "select"), ("/", "search")],
                    Mode::Operation => vec![
                        ("enter", "show"),
                        ("c", "checkout"),
                        ("d", "diff"),
                        ("r", "review"),
                        ("/", "search"),
                        ("?", "help"),
                    ],
                }
            };

            ShortcutsProps::default()
                .shortcuts(&shortcuts)
                .to_boxed_any()
                .into()
        });

    Page::default()
        .content(content)
        .shortcuts(shortcuts)
        .to_widget(tx.clone())
        .on_event(|key, _, props| {
            let default = PageProps::default();
            let props = props
                .and_then(|props| props.inner_ref::<PageProps>())
                .unwrap_or(&default);

            if props.handle_keys {
                match key {
                    Key::Char('q') | Key::Ctrl('c') => Some(Message::Quit),
                    Key::Char('?') => Some(Message::OpenHelp),
                    Key::Char('\n') => Some(Message::ExitFromMode),
                    Key::Char('c') => Some(Message::Exit {
                        operation: Some(PatchOperation::Checkout),
                    }),
                    Key::Char('d') => Some(Message::Exit {
                        operation: Some(PatchOperation::Diff),
                    }),
                    Key::Char('r') => Some(Message::Exit {
                        operation: Some(PatchOperation::Review),
                    }),
                    _ => None,
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

fn help_page(_state: &State, channel: &Channel<Message>) -> Widget<State, Message> {
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
        .to_widget(tx.clone());

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
        .on_event(|key, _, _| match key {
            Key::Esc | Key::Ctrl('c') => Some(Message::Quit),
            Key::Char('?') => Some(Message::LeavePage),
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
`Esc`:      Cancel
`q`:        Quit

# Specific keybindings

`enter`:    Select patch (if --mode id)
`enter`:    Show patch
`c`:        Checkout patch
`d`:        Show patch diff
`/`:        Search
`?`:        Show help

# Searching

Pattern:    is:<state> | is:authored | authors:[<did>, <did>] | <search>
Example:    is:open is:authored improve"#
        .into()
}
