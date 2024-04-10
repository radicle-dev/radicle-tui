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
use tui::ui::widget::container::{Footer, FooterProps, Header, HeaderProps};
use tui::ui::widget::input::{TextField, TextFieldProps};
use tui::ui::widget::text::{Paragraph, ParagraphProps};
use tui::ui::widget::{Column, Render, Shortcut, Shortcuts, ShortcutsProps, Table, Widget};
use tui::Selection;

use crate::tui_patch::common::Mode;
use crate::tui_patch::common::PatchOperation;

use super::{Action, State};

pub struct ListPageProps {
    mode: Mode,
    show_search: bool,
    show_help: bool,
}

impl From<&State> for ListPageProps {
    fn from(state: &State) -> Self {
        Self {
            mode: state.mode.clone(),
            show_search: state.ui.show_search,
            show_help: state.ui.show_help,
        }
    }
}

pub struct ListPage<'a> {
    /// Action sender
    pub action_tx: UnboundedSender<Action>,
    /// State mapped props
    props: ListPageProps,
    /// Notification widget
    patches: Patches,
    /// Search widget
    search: Search,
    /// Help widget
    help: Help<'a>,
    /// Shortcut widget
    shortcuts: Shortcuts<Action>,
}

impl<'a> Widget<State, Action> for ListPage<'a> {
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
            shortcuts: Shortcuts::new(state, action_tx),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        ListPage {
            patches: self.patches.move_with_state(state),
            search: self.search.move_with_state(state),
            shortcuts: self.shortcuts.move_with_state(state),
            help: self.help.move_with_state(state),
            props: ListPageProps::from(state),
            ..self
        }
    }

    fn handle_key_event(&mut self, key: termion::event::Key) {
        if self.props.show_search {
            <Search as Widget<State, Action>>::handle_key_event(&mut self.search, key)
        } else if self.props.show_help {
            <Help as Widget<State, Action>>::handle_key_event(&mut self.help, key)
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
                    <Patches as Widget<State, Action>>::handle_key_event(&mut self.patches, key);
                }
            }
        }
    }
}

impl<'a> Render<()> for ListPage<'a> {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, _area: Rect, _props: ()) {
        let area = frame.size();
        let layout = tui::ui::layout::default_page(area, 0u16, 1u16);

        let shortcuts = if self.props.show_search {
            vec![
                Shortcut::new("esc", "cancel"),
                Shortcut::new("enter", "apply"),
            ]
        } else if self.props.show_help {
            vec![Shortcut::new("?", "close")]
        } else {
            match self.props.mode {
                Mode::Id => vec![
                    Shortcut::new("enter", "select"),
                    Shortcut::new("/", "search"),
                ],
                Mode::Operation => vec![
                    Shortcut::new("enter", "show"),
                    Shortcut::new("c", "checkout"),
                    Shortcut::new("d", "diff"),
                    Shortcut::new("/", "search"),
                    Shortcut::new("?", "help"),
                ],
            }
        };

        if self.props.show_search {
            let component_layout = Layout::vertical([Constraint::Min(1), Constraint::Length(2)])
                .split(layout.component);

            self.patches.render::<B>(frame, component_layout[0], ());
            self.search
                .render::<B>(frame, component_layout[1], SearchProps {});
        } else if self.props.show_help {
            self.help.render::<B>(frame, layout.component, ());
        } else {
            self.patches.render::<B>(frame, layout.component, ());
        }

        self.shortcuts.render::<B>(
            frame,
            layout.shortcuts,
            ShortcutsProps {
                shortcuts,
                divider: '∙',
            },
        );
    }
}

#[derive(Clone)]
struct PatchesProps {
    mode: Mode,
    patches: Vec<PatchItem>,
    search: String,
    stats: HashMap<String, usize>,
    columns: Vec<Column>,
    cutoff: usize,
    cutoff_after: usize,
    focus: bool,
    page_size: usize,
    show_search: bool,
}

impl From<&State> for PatchesProps {
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

struct Patches {
    /// Action sender
    action_tx: UnboundedSender<Action>,
    /// State mapped props
    props: PatchesProps,
    /// Table header
    header: Header<Action>,
    /// Notification table
    table: Table<Action, PatchItem>,
    /// Table footer
    footer: Footer<Action>,
}

impl Widget<State, Action> for Patches {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self {
        let props = PatchesProps::from(state);

        Self {
            action_tx: action_tx.clone(),
            props: props.clone(),
            header: Header::new(state, action_tx.clone()),
            table: Table::new(state, action_tx.clone())
                .items(props.patches.clone())
                .columns(props.columns.to_vec())
                .header(true)
                .footer(!props.show_search)
                .cutoff(props.cutoff, props.cutoff_after),
            footer: Footer::new(state, action_tx),
        }
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        let props = PatchesProps::from(state);
        let patches: Vec<PatchItem> = state
            .patches
            .iter()
            .filter(|patch| state.filter.matches(patch))
            .cloned()
            .collect();

        let table = self
            .table
            .move_with_state(state)
            .items(patches)
            .footer(!state.ui.show_search)
            .page_size(state.ui.page_size);

        Self {
            props,
            header: self.header.move_with_state(state),
            table,
            footer: self.footer.move_with_state(state),
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
                <Table<Action, PatchItem> as Widget<State, Action>>::handle_key_event(
                    &mut self.table,
                    key,
                );
            }
        }
    }
}

impl Patches {
    fn render_header<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect) {
        self.header.render::<B>(
            frame,
            area,
            HeaderProps {
                columns: self.props.columns.clone(),
                focus: self.props.focus,
                cutoff: self.props.cutoff,
                cutoff_after: self.props.cutoff_after,
            },
        );
    }

    fn render_list<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect) {
        self.table.render::<B>(frame, area, ());
    }

    fn render_footer<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect) {
        let filter = PatchItemFilter::from_str(&self.props.search).unwrap_or_default();

        let search = Line::from(
            [
                span::default(" Search ".to_string())
                    .cyan()
                    .dim()
                    .reversed(),
                span::default(" ".into()),
                span::default(self.props.search.to_string()).gray().dim(),
            ]
            .to_vec(),
        );

        let draft = Line::from(
            [
                span::default(self.props.stats.get("Draft").unwrap_or(&0).to_string()).dim(),
                span::default(" Draft".to_string()).dim(),
            ]
            .to_vec(),
        );

        let open = Line::from(
            [
                span::positive(self.props.stats.get("Open").unwrap_or(&0).to_string()).dim(),
                span::default(" Open".to_string()).dim(),
            ]
            .to_vec(),
        );

        let merged = Line::from(
            [
                span::default(self.props.stats.get("Merged").unwrap_or(&0).to_string())
                    .magenta()
                    .dim(),
                span::default(" Merged".to_string()).dim(),
            ]
            .to_vec(),
        );

        let archived = Line::from(
            [
                span::default(self.props.stats.get("Archived").unwrap_or(&0).to_string())
                    .yellow()
                    .dim(),
                span::default(" Archived".to_string()).dim(),
            ]
            .to_vec(),
        );

        let sum = Line::from(
            [
                span::default("Σ ".to_string()).dim(),
                span::default(self.props.patches.len().to_string()).dim(),
            ]
            .to_vec(),
        );

        let progress = self
            .table
            .progress_percentage(self.props.patches.len(), self.props.page_size);
        let progress = span::default(format!("{}%", progress)).dim();

        match filter.status() {
            Some(state) => {
                let block = match state {
                    Status::Draft => draft,
                    Status::Open => open,
                    Status::Merged => merged,
                    Status::Archived => archived,
                };

                self.footer.render::<B>(
                    frame,
                    area,
                    FooterProps {
                        cells: [search.into(), block.clone().into(), progress.clone().into()],
                        widths: [
                            Constraint::Fill(1),
                            Constraint::Min(block.width() as u16),
                            Constraint::Min(4),
                        ],
                        focus: self.props.focus,
                        cutoff: self.props.cutoff,
                        cutoff_after: self.props.cutoff_after,
                    },
                );
            }
            None => {
                self.footer.render::<B>(
                    frame,
                    area,
                    FooterProps {
                        cells: [
                            search.into(),
                            draft.clone().into(),
                            open.clone().into(),
                            merged.clone().into(),
                            archived.clone().into(),
                            sum.clone().into(),
                            progress.clone().into(),
                        ],
                        widths: [
                            Constraint::Fill(1),
                            Constraint::Min(draft.width() as u16),
                            Constraint::Min(open.width() as u16),
                            Constraint::Min(merged.width() as u16),
                            Constraint::Min(archived.width() as u16),
                            Constraint::Min(sum.width() as u16),
                            Constraint::Min(4),
                        ],
                        focus: self.props.focus,
                        cutoff: self.props.cutoff,
                        cutoff_after: self.props.cutoff_after,
                    },
                );
            }
        };
    }
}

impl Render<()> for Patches {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let page_size = if self.props.show_search {
            let layout = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(area);

            self.render_header::<B>(frame, layout[0]);
            self.render_list::<B>(frame, layout[1]);

            layout[1].height as usize
        } else {
            let layout = Layout::vertical([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(area);

            self.render_header::<B>(frame, layout[0]);
            self.render_list::<B>(frame, layout[1]);
            self.render_footer::<B>(frame, layout[2]);

            layout[1].height as usize
        };

        if page_size != self.props.page_size {
            let _ = self.action_tx.send(Action::PageSize(page_size));
        }
    }
}

pub struct SearchProps {}

pub struct Search {
    pub action_tx: UnboundedSender<Action>,
    pub input: TextField,
}

impl Widget<State, Action> for Search {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let mut input = TextField::new(state, action_tx.clone());
        input.set_text(&state.search.read().to_string());

        Self { action_tx, input }.move_with_state(state)
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        let mut input = <TextField as Widget<State, Action>>::move_with_state(self.input, state);
        input.set_text(&state.search.read().to_string());

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
                <TextField as Widget<State, Action>>::handle_key_event(&mut self.input, key);
                let _ = self.action_tx.send(Action::UpdateSearch {
                    value: self.input.text().to_string(),
                });
            }
        }
    }
}

impl Render<SearchProps> for Search {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, _props: SearchProps) {
        let layout = Layout::horizontal(Constraint::from_mins([0]))
            .horizontal_margin(1)
            .split(area);

        self.input.render::<B>(
            frame,
            layout[0],
            TextFieldProps {
                titles: ("Search".into(), "Search".into()),
                show_cursor: true,
                inline_label: true,
            },
        );
    }
}

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
    header: Header<Action>,
    /// Content widget
    content: Paragraph<Action>,
    /// Container footer
    footer: Footer<Action>,
}

impl<'a> Widget<State, Action> for Help<'a> {
    fn new(state: &State, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
            props: HelpProps::from(state),
            header: Header::new(state, action_tx.clone()),
            content: Paragraph::new(state, action_tx.clone()),
            footer: Footer::new(state, action_tx),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &State) -> Self
    where
        Self: Sized,
    {
        Self {
            props: HelpProps::from(state),
            header: self.header.move_with_state(state),
            content: self.content.move_with_state(state),
            footer: self.footer.move_with_state(state),
            ..self
        }
    }

    fn handle_key_event(&mut self, key: termion::event::Key) {
        let len = self.props.content.lines.len() + 1;
        let page_size = self.props.page_size;
        match key {
            Key::Esc => {
                let _ = self.action_tx.send(Action::Exit { selection: None });
            }
            Key::Char('?') => {
                let _ = self.action_tx.send(Action::CloseHelp);
            }
            Key::Up | Key::Char('k') => {
                self.content.prev(len, page_size);
            }
            Key::Down | Key::Char('j') => {
                self.content.next(len, page_size);
            }
            Key::PageUp => {
                self.content.prev_page(len, page_size);
            }
            Key::PageDown => {
                self.content.next_page(len, page_size);
            }
            Key::Home => {
                self.content.begin(len, page_size);
            }
            Key::End => {
                self.content.end(len, page_size);
            }
            _ => {}
        }
    }
}

impl<'a> Render<()> for Help<'a> {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, _props: ()) {
        let [header_area, content_area, footer_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .areas(area);

        self.header.render::<B>(
            frame,
            header_area,
            HeaderProps {
                columns: [Column::new(" Help ", Constraint::Fill(1))].to_vec(),
                focus: self.props.focus,
                cutoff: usize::MIN,
                cutoff_after: usize::MAX,
            },
        );

        self.content.render::<B>(
            frame,
            content_area,
            ParagraphProps {
                content: self.props.content.clone(),
                focus: self.props.focus,
                has_footer: true,
                has_header: true,
            },
        );

        let progress = span::default(format!("{}%", self.content.progress())).dim();

        self.footer.render::<B>(
            frame,
            footer_area,
            FooterProps {
                cells: [String::new().into(), progress.clone().into()],
                widths: [Constraint::Fill(1), Constraint::Min(4)],
                focus: self.props.focus,
                cutoff: usize::MAX,
                cutoff_after: usize::MAX,
            },
        );

        let page_size = content_area.height as usize;
        if page_size != self.props.page_size {
            let _ = self.action_tx.send(Action::PageSize(page_size));
        }
    }
}
