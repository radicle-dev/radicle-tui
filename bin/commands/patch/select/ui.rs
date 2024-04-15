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

use tui::ui::items::{Filter, PatchItem, PatchItemFilter};
use tui::ui::span;
use tui::ui::widget::container::{Footer, Header};
use tui::ui::widget::input::TextField;
use tui::ui::widget::text::Paragraph;
use tui::ui::widget::{Column, Render, Shortcuts, Table, View, Widget};
use tui::Selection;

use crate::tui_patch::common::Mode;
use crate::tui_patch::common::PatchOperation;

use super::{Action, State};

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

impl<'a, B: Backend> Widget<State, Action, B> for Patches<'a> {}
impl<B: Backend> Widget<State, Action, B> for Search {}

pub struct ListPage<'a> {
    /// Action sender
    pub action_tx: UnboundedSender<Action>,
    /// State mapped props
    props: ListPageProps,
    /// Notification widget
    patches: Patches<'a>,
    /// Search widget
    search: Search,
    /// Help widget
    help: Help<'a>,
    /// Shortcut widget
    shortcuts: Shortcuts<Action>,
}

impl<'a> View<State, Action> for ListPage<'a> {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
            props: ListPageProps::from(state),
            patches: Patches::new(state, action_tx.clone()),
            search: Search::new(state, action_tx.clone()),
            help: Help::new(state, action_tx.clone()),
            shortcuts: Shortcuts::new(&(), action_tx),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        let shorts = if state.ui.show_search {
            vec![("esc", "cancel"), ("enter", "apply")]
        } else if state.ui.show_help {
            vec![("?", "close")]
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

        let shortcuts = self.shortcuts.move_with_state(state);
        let shortcuts = shortcuts.shortcuts(&shorts);

        ListPage {
            patches: self.patches.move_with_state(state),
            search: self.search.move_with_state(state),
            shortcuts,
            help: self.help.move_with_state(state),
            props: ListPageProps::from(state),
            ..self
        }
    }

    fn handle_key_event(&mut self, key: termion::event::Key) {
        if self.props.show_search {
            <Search as View<State, Action>>::handle_key_event(&mut self.search, key)
        } else if self.props.show_help {
            <Help as View<State, Action>>::handle_key_event(&mut self.help, key);
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
                    <Patches as View<State, Action>>::handle_key_event(&mut self.patches, key);
                }
            }
        }
        let _ = self.action_tx.send(Action::Update);
    }
}

impl<'a, B: Backend> Render<B, ()> for ListPage<'a> {
    fn render(&self, frame: &mut ratatui::Frame, _area: Rect, _props: ()) {
        let area = frame.size();
        let layout = tui::ui::layout::default_page(area, 0u16, 1u16);

        if self.props.show_search {
            let component_layout = Layout::vertical([Constraint::Min(1), Constraint::Length(2)])
                .split(layout.component);

            <Patches<'_> as Render<B, ()>>::render(&self.patches, frame, component_layout[0], ());
            <Search as Render<B, ()>>::render(&self.search, frame, component_layout[1], ());
        } else if self.props.show_help {
            <Help<'_> as Render<B, ()>>::render(&self.help, frame, layout.component, ());
        } else {
            <Patches<'_> as Render<B, ()>>::render(&self.patches, frame, layout.component, ());
        }

        <Shortcuts<_> as Render<B, ()>>::render(&self.shortcuts, frame, layout.shortcuts, ());
    }
}

#[derive(Clone)]
struct PatchesProps<'a> {
    mode: Mode,
    patches: Vec<PatchItem>,
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

        let patches: Vec<PatchItem> = state
            .patches
            .iter()
            .filter(|patch| state.filter.matches(patch))
            .cloned()
            .collect();

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
        }
    }
}

struct Patches<'a> {
    /// Action sender
    action_tx: UnboundedSender<Action>,
    /// State mapped props
    props: PatchesProps<'a>,
    /// Notification table
    table: Table<'a, Action, PatchItem>,
    /// Table footer
    footer: Footer<'a, Action>,
}

impl<'a> View<State, Action> for Patches<'a> {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self {
        let props = PatchesProps::from(state);

        Self {
            action_tx: action_tx.clone(),
            props: props.clone(),
            table: Table::new(&(), action_tx.clone())
                .items(props.patches.clone())
                .columns(props.columns.to_vec())
                .header(
                    Header::new(&(), action_tx.clone())
                        .columns(props.columns.clone())
                        .cutoff(props.cutoff, props.cutoff_after)
                        .focus(props.focus),
                )
                .footer(!props.show_search)
                .cutoff(props.cutoff, props.cutoff_after),
            footer: Footer::new(&(), action_tx),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &State) -> Self {
        let patches: Vec<PatchItem> = state
            .patches
            .iter()
            .filter(|patch| state.filter.matches(patch))
            .cloned()
            .collect();

        let props = PatchesProps::from(state);

        let table = self
            .table
            .move_with_state(&())
            .items(patches)
            .footer(!state.ui.show_search)
            .page_size(state.ui.page_size);

        let footer = self.footer.move_with_state(&());
        let footer = footer.columns(Self::build_footer(&props, table.selected()));

        Self {
            props,
            table,
            footer,
            ..self
        }
    }

    fn handle_key_event(&mut self, key: Key) {
        match key {
            Key::Char('\n') => {
                let operation = match self.props.mode {
                    Mode::Operation => Some(PatchOperation::Show.to_string()),
                    Mode::Id => None,
                };

                self.table
                    .selected()
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
                self.table
                    .selected()
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
                self.table
                    .selected()
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
                <Table<Action, PatchItem> as View<(), Action>>::handle_key_event(
                    &mut self.table,
                    key,
                );
            }
        }
    }
}

impl<'a> Patches<'a> {
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
            .map(|selected| {
                Table::<Action, PatchItem>::progress(selected, props.patches.len(), props.page_size)
            })
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

impl<'a, B: Backend> Render<B, ()> for Patches<'a> {
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let header_height = 3_usize;

        let page_size = if self.props.show_search {
            <Table<'_, _, _> as Render<B, ()>>::render(&self.table, frame, area, ());

            (area.height as usize).saturating_sub(header_height)
        } else {
            let layout = Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(area);

            <Table<'_, _, _> as Render<B, ()>>::render(&self.table, frame, layout[0], ());
            <Footer<'_, _> as Render<B, ()>>::render(&self.footer, frame, layout[1], ());

            (area.height as usize).saturating_sub(header_height)
        };

        if page_size != self.props.page_size {
            let _ = self.action_tx.send(Action::PageSize(page_size));
        }
    }
}

pub struct Search {
    pub action_tx: UnboundedSender<Action>,
    pub input: TextField<Action>,
}

impl View<State, Action> for Search {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let input = TextField::new(state, action_tx.clone())
            .title("Search")
            .inline(true);
        Self { action_tx, input }.move_with_state(state)
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        let input = self.input.move_with_state(state);
        let input = input.text(&state.search.read().to_string());

        Self { input, ..self }
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
                <TextField<Action> as View<State, Action>>::handle_key_event(&mut self.input, key);
                let _ = self.action_tx.send(Action::UpdateSearch {
                    value: self.input.read().to_string(),
                });
            }
        }
    }
}

impl<B: Backend> Render<B, ()> for Search {
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let layout = Layout::horizontal(Constraint::from_mins([0]))
            .horizontal_margin(1)
            .split(area);

        <TextField<_> as Render<B, ()>>::render(&self.input, frame, layout[0], ());
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

pub struct Help<'a> {
    /// Send messages
    pub action_tx: UnboundedSender<Action>,
    /// This widget's render properties
    pub props: HelpProps<'a>,
    /// Container header
    header: Header<'a, Action>,
    /// Content widget
    content: Paragraph<'a, Action>,
    /// Container footer
    footer: Footer<'a, Action>,
}

impl<'a> View<State, Action> for Help<'a> {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let props = HelpProps::from(state);

        Self {
            action_tx: action_tx.clone(),
            props: props.clone(),
            header: Header::new(&(), action_tx.clone()),
            content: Paragraph::new(state, action_tx.clone()),
            footer: Footer::new(&(), action_tx),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        let props = HelpProps::from(state);

        let header = self.header.move_with_state(&());
        let header = header
            .columns([Column::new(" Help ", Constraint::Fill(1))].to_vec())
            .focus(props.focus);

        let content = self.content.move_with_state(state);
        let content = content.text(&props.content).page_size(props.page_size);

        let progress = span::default(format!("{}%", content.progress())).dim();

        let footer = self.footer.move_with_state(&());
        let footer = footer
            .columns(
                [
                    Column::new(Text::raw(""), Constraint::Fill(1)),
                    Column::new(Text::from(progress), Constraint::Min(4)),
                ]
                .to_vec(),
            )
            .focus(props.focus);

        Self {
            props,
            header,
            content,
            footer,
            ..self
        }
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
                <Paragraph<_> as View<(), _>>::handle_key_event(&mut self.content, key);
            }
        }
    }
}

impl<'a, B: Backend> Render<B, ()> for Help<'a> {
    fn render(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let [header_area, content_area, footer_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .areas(area);

        <Header<'_, _> as Render<B, ()>>::render(&self.header, frame, header_area, ());
        <Paragraph<'_, _> as Render<B, ()>>::render(&self.content, frame, content_area, ());
        <Footer<'_, _> as Render<B, ()>>::render(&self.footer, frame, footer_area, ());

        let page_size = content_area.height as usize;
        if page_size != self.props.page_size {
            let _ = self.action_tx.send(Action::PageSize(page_size));
        }
    }
}
