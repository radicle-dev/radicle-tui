use std::any::Any;
use std::collections::HashMap;
use std::str::FromStr;
use std::vec;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};

use radicle::patch::{self, Status};

use radicle_tui as tui;

use tui::ui::items::{PatchItem, PatchItemFilter};
use tui::ui::span;
use tui::ui::widget::container::{Footer, FooterProps, Header, HeaderProps};
use tui::ui::widget::input::{TextField, TextFieldProps};
use tui::ui::widget::text::{Paragraph, ParagraphProps};
use tui::ui::widget::{self, TableUtils};
use tui::ui::widget::{
    Column, EventCallback, Shortcuts, ShortcutsProps, Table, TableProps, UpdateCallback, View,
    Widget,
};
use tui::Selection;

use crate::tui_patch::common::Mode;
use crate::tui_patch::common::PatchOperation;

use super::{Action, State};

type BoxedWidget<B> = widget::BoxedWidget<B, State, Action>;

pub struct ListPageProps {
    show_search: bool,
    show_help: bool,
}

impl From<&State> for ListPageProps {
    fn from(state: &State) -> Self {
        Self {
            show_search: state.ui.show_search,
            show_help: state.ui.show_help,
        }
    }
}

pub struct ListPage<B: Backend> {
    /// Internal properties
    props: ListPageProps,
    /// Message sender
    action_tx: UnboundedSender<Action>,
    /// Custom update handler
    on_update: Option<UpdateCallback<State>>,
    /// Additional custom event handler
    on_change: Option<EventCallback<Action>>,
    /// Patches widget
    patches: BoxedWidget<B>,
    /// Search widget
    search: BoxedWidget<B>,
    /// Help widget
    help: BoxedWidget<B>,
    /// Shortcut widget
    shortcuts: BoxedWidget<B>,
}

impl<'a: 'static, B: Backend + 'a> View<State, Action> for ListPage<B> {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
            props: ListPageProps::from(state),
            patches: Patches::new(state, action_tx.clone()).to_boxed(),
            search: Search::new(state, action_tx.clone()).to_boxed(),
            help: Help::new(state, action_tx.clone()).to_boxed(),
            shortcuts: Shortcuts::new(state, action_tx.clone())
                .on_update(|state| {
                    ShortcutsProps::default()
                        .shortcuts(&state.shortcuts())
                        .to_boxed()
                })
                .to_boxed(),
            on_update: None,
            on_change: None,
        }
    }

    fn on_update(mut self, callback: UpdateCallback<State>) -> Self {
        self.on_update = Some(callback);
        self
    }

    fn on_change(mut self, callback: EventCallback<Action>) -> Self {
        self.on_change = Some(callback);
        self
    }

    fn update(&mut self, state: &State) {
        self.props = ListPageProps::from(state);

        self.patches.update(state);
        self.search.update(state);
        self.help.update(state);
        self.shortcuts.update(state);
    }

    fn handle_key_event(&mut self, key: termion::event::Key) {
        if self.props.show_search {
            self.search.handle_key_event(key);
        } else if self.props.show_help {
            self.help.handle_key_event(key);
        } else {
            match key {
                Key::Esc | Key::Ctrl('c') => {
                    let _ = self.action_tx.send(Action::Exit { selection: None });
                }
                Key::Char('/') => {
                    let _ = self.action_tx.send(Action::OpenSearch);
                }
                Key::Char('?') => {
                    let _ = self.action_tx.send(Action::OpenHelp);
                }
                _ => {
                    self.patches.handle_key_event(key);
                }
            }
        }
    }
}

impl<'a: 'static, B> Widget<B, State, Action> for ListPage<B>
where
    B: Backend + 'a,
{
    fn render(&self, frame: &mut ratatui::Frame, _area: Rect, __props: &dyn Any) {
        let area = frame.size();
        let layout = tui::ui::layout::default_page(area, 0u16, 1u16);

        if self.props.show_search {
            let component_layout = Layout::vertical([Constraint::Min(1), Constraint::Length(2)])
                .split(layout.component);

            self.patches.render(frame, component_layout[0], &());
            self.search.render(frame, component_layout[1], &());
        } else if self.props.show_help {
            self.help.render(frame, layout.component, &());
        } else {
            self.patches.render(frame, layout.component, &());
        }

        self.shortcuts.render(frame, layout.shortcuts, &());
    }
}

#[derive(Clone)]
struct PatchesProps<'a> {
    mode: Mode,
    patches: Vec<PatchItem>,
    selected: Option<usize>,
    search: String,
    stats: HashMap<String, usize>,
    columns: Vec<Column<'a>>,
    cutoff: usize,
    cutoff_after: usize,
    focus: bool,
    page_size: usize,
    show_search: bool,
}

impl<'a> From<&State> for PatchesProps<'a> {
    fn from(state: &State) -> Self {
        let mut draft = 0;
        let mut open = 0;
        let mut archived = 0;
        let mut merged = 0;

        let patches = state.patches();

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
            search: state.search.read(),
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
            focus: false,
            stats,
            page_size: state.ui.page_size,
            show_search: state.ui.show_search,
            selected: state.patches.selected,
        }
    }
}

struct Patches<'a, B> {
    /// Internal properties
    props: PatchesProps<'a>,
    /// Message sender
    action_tx: UnboundedSender<Action>,
    /// Custom update handler
    on_update: Option<UpdateCallback<State>>,
    /// Additional custom event handler
    on_change: Option<EventCallback<Action>>,
    /// Table widget
    table: BoxedWidget<B>,
    /// Footer widget w/ context
    footer: BoxedWidget<B>,
}

impl<'a: 'static, B> View<State, Action> for Patches<'a, B>
where
    B: Backend + 'a,
{
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self {
        let props = PatchesProps::from(state);

        Self {
            action_tx: action_tx.clone(),
            props: props.clone(),
            table: Box::<Table<B, State, Action, PatchItem>>::new(
                Table::new(state, action_tx.clone())
                    .header(
                        Header::new(state, action_tx.clone())
                            .columns(props.columns.clone())
                            .cutoff(props.cutoff, props.cutoff_after)
                            .focus(props.focus)
                            .to_boxed(),
                    )
                    .on_change(|props, action_tx| {
                        props
                            .downcast_ref::<TableProps<'_, PatchItem>>()
                            .and_then(|props| {
                                action_tx
                                    .send(Action::Select {
                                        selected: props.selected,
                                    })
                                    .ok()
                            });
                    })
                    .on_update(|state| {
                        let props = PatchesProps::from(state);

                        TableProps::default()
                            .columns(props.columns)
                            .items(state.patches())
                            .footer(!state.ui.show_search)
                            .page_size(state.ui.page_size)
                            .cutoff(props.cutoff, props.cutoff_after)
                            .to_boxed()
                    }),
            ),
            footer: Footer::new(state, action_tx).to_boxed(),
            on_update: None,
            on_change: None,
        }
    }

    fn on_update(mut self, callback: UpdateCallback<State>) -> Self {
        self.on_update = Some(callback);
        self
    }

    fn on_change(mut self, callback: EventCallback<Action>) -> Self {
        self.on_change = Some(callback);
        self
    }

    fn update(&mut self, state: &State) {
        // TODO call mapper here instead?
        self.props = PatchesProps::from(state);

        self.table.update(state);
        self.footer.update(state);
    }

    fn handle_key_event(&mut self, key: Key) {
        match key {
            Key::Char('\n') => {
                let operation = match self.props.mode {
                    Mode::Operation => Some(PatchOperation::Show.to_string()),
                    Mode::Id => None,
                };

                self.props
                    .selected
                    .and_then(|selected| self.props.patches.get(selected))
                    .and_then(|patch| {
                        self.action_tx
                            .send(Action::Exit {
                                selection: Some(Selection {
                                    operation,
                                    ids: vec![patch.id],
                                    args: vec![],
                                }),
                            })
                            .ok()
                    });
            }
            Key::Char('c') => {
                self.props
                    .selected
                    .and_then(|selected| self.props.patches.get(selected))
                    .and_then(|patch| {
                        self.action_tx
                            .send(Action::Exit {
                                selection: Some(Selection {
                                    operation: Some(PatchOperation::Checkout.to_string()),
                                    ids: vec![patch.id],
                                    args: vec![],
                                }),
                            })
                            .ok()
                    });
            }
            Key::Char('d') => {
                self.props
                    .selected
                    .and_then(|selected| self.props.patches.get(selected))
                    .and_then(|patch| {
                        self.action_tx
                            .send(Action::Exit {
                                selection: Some(Selection {
                                    operation: Some(PatchOperation::Diff.to_string()),
                                    ids: vec![patch.id],
                                    args: vec![],
                                }),
                            })
                            .ok()
                    });
            }
            _ => {
                self.table.handle_key_event(key);
            }
        }
    }
}

impl<'a, B: Backend> Patches<'a, B> {
    fn build_footer(props: &PatchesProps<'a>, selected: Option<usize>) -> Vec<Column<'a>> {
        let filter = PatchItemFilter::from_str(&props.search).unwrap_or_default();

        let search = Line::from(
            [
                span::default(" Search ".to_string())
                    .cyan()
                    .dim()
                    .reversed(),
                span::default(" ".into()),
                span::default(props.search.to_string()).gray().dim(),
            ]
            .to_vec(),
        );

        let draft = Line::from(
            [
                span::default(props.stats.get("Draft").unwrap_or(&0).to_string()).dim(),
                span::default(" Draft".to_string()).dim(),
            ]
            .to_vec(),
        );

        let open = Line::from(
            [
                span::positive(props.stats.get("Open").unwrap_or(&0).to_string()).dim(),
                span::default(" Open".to_string()).dim(),
            ]
            .to_vec(),
        );

        let merged = Line::from(
            [
                span::default(props.stats.get("Merged").unwrap_or(&0).to_string())
                    .magenta()
                    .dim(),
                span::default(" Merged".to_string()).dim(),
            ]
            .to_vec(),
        );

        let archived = Line::from(
            [
                span::default(props.stats.get("Archived").unwrap_or(&0).to_string())
                    .yellow()
                    .dim(),
                span::default(" Archived".to_string()).dim(),
            ]
            .to_vec(),
        );

        let sum = Line::from(
            [
                span::default("Σ ".to_string()).dim(),
                span::default(props.patches.len().to_string()).dim(),
            ]
            .to_vec(),
        );

        let progress = selected
            .map(|selected| TableUtils::progress(selected, props.patches.len(), props.page_size))
            .unwrap_or_default();
        let progress = span::default(format!("{}%", progress)).dim();

        match filter.status() {
            Some(state) => {
                let block = match state {
                    Status::Draft => draft,
                    Status::Open => open,
                    Status::Merged => merged,
                    Status::Archived => archived,
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
            ]
            .to_vec(),
        }
    }
}

impl<'a: 'static, B> Widget<B, State, Action> for Patches<'a, B>
where
    B: Backend + 'a,
{
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, props: &dyn Any) {
        let props = props.downcast_ref::<PatchesProps>().unwrap_or(&self.props);

        let header_height = 3_usize;

        let page_size = if props.show_search {
            self.table.render(frame, area, &());

            (area.height as usize).saturating_sub(header_height)
        } else {
            let layout = Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(area);

            self.table.render(frame, layout[0], &());
            self.footer.render(
                frame,
                layout[1],
                &FooterProps::default().columns(Self::build_footer(props, props.selected)),
            );

            (area.height as usize).saturating_sub(header_height)
        };

        if page_size != props.page_size {
            let _ = self.action_tx.send(Action::PageSize(page_size));
        }
    }
}

pub struct Search<B: Backend> {
    /// Message sender
    action_tx: UnboundedSender<Action>,
    /// Custom update handler
    on_update: Option<UpdateCallback<State>>,
    /// Additional custom event handler
    on_change: Option<EventCallback<Action>>,
    /// Search input field
    input: BoxedWidget<B>,
}

impl<B: Backend> View<State, Action> for Search<B> {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let input = TextField::new(state, action_tx.clone())
            .on_change(|props, action_tx| {
                props.downcast_ref::<TextFieldProps>().and_then(|props| {
                    action_tx
                        .send(Action::UpdateSearch {
                            value: props.text.clone(),
                        })
                        .ok()
                });
            })
            .on_update(|state| {
                TextFieldProps::default()
                    .text(&state.search.read().to_string())
                    .title("Search")
                    .inline(true)
                    .to_boxed()
            })
            .to_boxed();
        Self {
            action_tx,
            input,
            on_update: None,
            on_change: None,
        }
    }

    fn on_update(mut self, callback: UpdateCallback<State>) -> Self {
        self.on_update = Some(callback);
        self
    }

    fn on_change(mut self, callback: EventCallback<Action>) -> Self {
        self.on_change = Some(callback);
        self
    }

    fn update(&mut self, state: &State) {
        self.input.update(state);
    }

    fn handle_key_event(&mut self, key: termion::event::Key) {
        match key {
            Key::Esc => {
                let _ = self.action_tx.send(Action::CloseSearch);
            }
            Key::Char('\n') => {
                let _ = self.action_tx.send(Action::ApplySearch);
            }
            _ => {
                self.input.handle_key_event(key);
            }
        }
    }
}

impl<B> Widget<B, State, Action> for Search<B>
where
    B: Backend,
{
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, _props: &dyn Any) {
        let layout = Layout::horizontal(Constraint::from_mins([0]))
            .horizontal_margin(1)
            .split(area);

        self.input.render(frame, layout[0], &());
    }
}

#[derive(Clone)]
pub struct HelpProps<'a> {
    content: Text<'a>,
    focus: bool,
    page_size: usize,
}

impl<'a> From<&State> for HelpProps<'a> {
    fn from(state: &State) -> Self {
        let content = Text::from(
            [
                Line::from(Span::raw("Generic keybindings").cyan()),
                Line::raw(""),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "↑,k")).gray(),
                        Span::raw(" "),
                        Span::raw("move cursor one line up").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "↓,j")).gray(),
                        Span::raw(" "),
                        Span::raw("move cursor one line down").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "PageUp")).gray(),
                        Span::raw(" "),
                        Span::raw("move cursor one page up").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "PageDown")).gray(),
                        Span::raw(" "),
                        Span::raw("move cursor one page down").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "Home")).gray(),
                        Span::raw(" "),
                        Span::raw("move cursor to the first line").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "End")).gray(),
                        Span::raw(" "),
                        Span::raw("move cursor to the last line").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::raw(""),
                Line::from(Span::raw("Specific keybindings").cyan()),
                Line::raw(""),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "enter")).gray(),
                        Span::raw(" "),
                        Span::raw("Select patch (if --mode id)").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "enter")).gray(),
                        Span::raw(" "),
                        Span::raw("Show patch").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "c")).gray(),
                        Span::raw(" "),
                        Span::raw("Checkout patch").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "d")).gray(),
                        Span::raw(" "),
                        Span::raw("Show patch diff").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "/")).gray(),
                        Span::raw(" "),
                        Span::raw("Search").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "?")).gray(),
                        Span::raw(" "),
                        Span::raw("Show help").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "Esc")).gray(),
                        Span::raw(" "),
                        Span::raw("Quit / cancel").gray().dim(),
                    ]
                    .to_vec(),
                ),
                Line::raw(""),
                Line::from(Span::raw("Searching").cyan()),
                Line::raw(""),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "Pattern")).gray(),
                        Span::raw(" "),
                        Span::raw("is:<state> | is:authored | authors:[<did>, <did>] | <search>")
                            .gray()
                            .dim(),
                    ]
                    .to_vec(),
                ),
                Line::from(
                    [
                        Span::raw(format!("{key:>10}", key = "Example")).gray(),
                        Span::raw(" "),
                        Span::raw("is:open is:authored improve").gray().dim(),
                    ]
                    .to_vec(),
                ),
            ]
            .to_vec(),
        );

        Self {
            content,
            focus: false,
            page_size: state.ui.page_size,
        }
    }
}

pub struct Help<'a, B: Backend> {
    /// Internal properties
    props: HelpProps<'a>,
    /// Message sender
    action_tx: UnboundedSender<Action>,
    /// Custom update handler
    on_update: Option<UpdateCallback<State>>,
    /// Additional custom event handler
    on_change: Option<EventCallback<Action>>,
    /// Container header
    header: BoxedWidget<B>,
    /// Content widget
    content: BoxedWidget<B>,
    /// Container footer
    footer: BoxedWidget<B>,
}

impl<'a, B: Backend> View<State, Action> for Help<'a, B> {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
            props: HelpProps::from(state),
            header: Header::new(state, action_tx.clone()).to_boxed(),
            content: Paragraph::new(state, action_tx.clone())
                .on_update(|state| {
                    let props = HelpProps::from(state);

                    ParagraphProps::default()
                        .text(&props.content)
                        .page_size(props.page_size)
                        .focus(props.focus)
                        .to_boxed()
                })
                .to_boxed(),
            footer: Footer::new(state, action_tx).to_boxed(),
            on_update: None,
            on_change: None,
        }
    }

    fn on_update(mut self, callback: UpdateCallback<State>) -> Self {
        self.on_update = Some(callback);
        self
    }

    fn on_change(mut self, callback: EventCallback<Action>) -> Self {
        self.on_change = Some(callback);
        self
    }

    fn update(&mut self, state: &State) {
        self.props = HelpProps::from(state);

        self.header.update(state);
        self.content.update(state);
        self.footer.update(state);
    }

    fn handle_key_event(&mut self, key: termion::event::Key) {
        match key {
            Key::Esc => {
                let _ = self.action_tx.send(Action::Exit { selection: None });
            }
            Key::Char('?') => {
                let _ = self.action_tx.send(Action::CloseHelp);
            }
            _ => {
                self.content.handle_key_event(key);
            }
        }
    }
}

impl<'a: 'static, B> Widget<B, State, Action> for Help<'a, B>
where
    B: Backend,
{
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, props: &dyn Any) {
        let props = props.downcast_ref::<HelpProps<'_>>().unwrap_or(&self.props);

        let [header_area, content_area, footer_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .areas(area);
        let progress = span::default(format!("{}%", 0)).dim();

        self.header.render(
            frame,
            header_area,
            &HeaderProps::default()
                .columns([Column::new(" Help ", Constraint::Fill(1))].to_vec())
                .focus(props.focus),
        );

        self.content.render(
            frame,
            content_area,
            &ParagraphProps::default()
                .text(&props.content)
                .page_size(props.page_size)
                .focus(props.focus),
        );

        self.footer.render(
            frame,
            footer_area,
            &FooterProps::default()
                .columns(
                    [
                        Column::new(Text::raw(""), Constraint::Fill(1)),
                        Column::new(progress, Constraint::Min(4)),
                    ]
                    .to_vec(),
                )
                .focus(props.focus),
        );

        let page_size = content_area.height as usize;
        if page_size != props.page_size {
            let _ = self.action_tx.send(Action::PageSize(page_size));
        }
    }
}
