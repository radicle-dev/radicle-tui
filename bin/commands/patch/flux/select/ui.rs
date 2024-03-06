use std::collections::HashMap;
use std::str::FromStr;
use std::vec;

use radicle::patch::{self, Status};

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;

use radicle_tui as tui;

use tui::flux::store::StateValue;
use tui::flux::ui::cob::{PatchItem, PatchItemFilter};
use tui::flux::ui::span;
use tui::flux::ui::widget::container::{Footer, FooterProps, Header, HeaderProps};
use tui::flux::ui::widget::input_box;
use tui::flux::ui::widget::input_box::InputBox;
use tui::flux::ui::widget::{
    Render, Shortcut, Shortcuts, ShortcutsProps, Table, TableProps, Widget,
};
use tui::Selection;

use crate::tui_patch::common::Mode;
use crate::tui_patch::common::PatchOperation;

use super::{Action, PatchesState};

pub struct ListPageProps {
    selected: Option<PatchItem>,
    mode: Mode,
    show_search: bool,
}

impl From<&PatchesState> for ListPageProps {
    fn from(state: &PatchesState) -> Self {
        Self {
            selected: state.selected.clone(),
            mode: state.mode.clone(),
            show_search: state.ui.show_search,
        }
    }
}

pub struct ListPage {
    /// Action sender
    pub action_tx: UnboundedSender<Action>,
    /// State mapped props
    props: ListPageProps,
    /// Notification widget
    patches: Patches,
    /// Search widget
    search: Search,
    /// Shortcut widget
    shortcuts: Shortcuts<Action>,
}

impl Widget<PatchesState, Action> for ListPage {
    fn new(state: &PatchesState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
            props: ListPageProps::from(state),
            patches: Patches::new(state, action_tx.clone()),
            search: Search::new(state, action_tx.clone()),
            shortcuts: Shortcuts::new(state, action_tx),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &PatchesState) -> Self
    where
        Self: Sized,
    {
        ListPage {
            patches: self.patches.move_with_state(state),
            search: self.search.move_with_state(state),
            shortcuts: self.shortcuts.move_with_state(state),
            props: ListPageProps::from(state),
            ..self
        }
    }

    fn name(&self) -> &str {
        "list-page"
    }

    fn handle_key_event(&mut self, key: termion::event::Key) {
        if self.props.show_search {
            <Search as Widget<PatchesState, Action>>::handle_key_event(&mut self.search, key)
        } else {
            match key {
                Key::Esc | Key::Ctrl('c') => {
                    let _ = self.action_tx.send(Action::Exit { selection: None });
                }
                Key::Char('\n') => {
                    if let Some(selected) = &self.props.selected {
                        let operation = match self.props.mode {
                            Mode::Operation => Some(PatchOperation::Show.to_string()),
                            Mode::Id => None,
                        };
                        let _ = self.action_tx.send(Action::Exit {
                            selection: Some(Selection {
                                operation,
                                ids: vec![selected.id],
                                args: vec![],
                            }),
                        });
                    }
                }
                Key::Char('c') => {
                    if let Some(selected) = &self.props.selected {
                        let selection = Selection {
                            operation: Some(PatchOperation::Checkout.to_string()),
                            ids: vec![selected.id],
                            args: vec![],
                        };
                        let _ = self.action_tx.send(Action::Exit {
                            selection: Some(selection),
                        });
                    }
                }
                Key::Char('d') => {
                    if let Some(selected) = &self.props.selected {
                        let selection = Selection {
                            operation: Some(PatchOperation::Diff.to_string()),
                            ids: vec![selected.id],
                            args: vec![],
                        };
                        let _ = self.action_tx.send(Action::Exit {
                            selection: Some(selection),
                        });
                    }
                }
                Key::Char('/') => {
                    let _ = self.action_tx.send(Action::OpenSearch);
                }
                _ => {
                    <Patches as Widget<PatchesState, Action>>::handle_key_event(
                        &mut self.patches,
                        key,
                    );
                }
            }
        }
    }
}

impl Render<()> for ListPage {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, _area: Rect, _props: ()) {
        let area = frame.size();
        let layout = tui::flux::ui::layout::default_page(area, 0u16, 1u16);

        let shortcuts = if self.props.show_search {
            vec![
                Shortcut::new("esc", "back"),
                Shortcut::new("enter", "search"),
            ]
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
                ],
            }
        };

        if self.props.show_search {
            let component_layout = Layout::vertical([Constraint::Min(1), Constraint::Length(2)])
                .split(layout.component);

            self.patches.render::<B>(frame, component_layout[0], ());
            self.search
                .render::<B>(frame, component_layout[1], SearchProps {});
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

struct PatchesProps {
    patches: Vec<PatchItem>,
    search: StateValue<String>,
    stats: HashMap<String, usize>,
    widths: [Constraint; 9],
    cutoff: usize,
    cutoff_after: usize,
    focus: bool,
    page_size: usize,
    show_search: bool,
}

impl From<&PatchesState> for PatchesProps {
    fn from(state: &PatchesState) -> Self {
        let mut draft = 0;
        let mut open = 0;
        let mut archived = 0;
        let mut merged = 0;

        let filter = PatchItemFilter::from_str(&state.search.read()).unwrap_or_default();
        let mut patches = state
            .patches
            .clone()
            .into_iter()
            .filter(|patch| filter.matches(patch))
            .collect::<Vec<_>>();

        // Apply sorting
        patches.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

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
            patches,
            search: state.search.clone(),
            widths: [
                Constraint::Length(3),
                Constraint::Length(8),
                Constraint::Fill(1),
                Constraint::Length(16),
                Constraint::Length(16),
                Constraint::Length(8),
                Constraint::Length(6),
                Constraint::Length(6),
                Constraint::Length(16),
            ],
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
    table: Table<Action>,
    /// Table footer
    footer: Footer<Action>,
}

impl Widget<PatchesState, Action> for Patches {
    fn new(state: &PatchesState, action_tx: UnboundedSender<Action>) -> Self {
        Self {
            action_tx: action_tx.clone(),
            props: PatchesProps::from(state),
            header: Header::new(state, action_tx.clone()),
            table: Table::new(state, action_tx.clone()),
            footer: Footer::new(state, action_tx),
        }
    }

    fn move_with_state(self, state: &PatchesState) -> Self
    where
        Self: Sized,
    {
        let props = PatchesProps::from(state);
        let mut table = self.table.move_with_state(state);

        if let Some(selected) = table.selected() {
            if selected > props.patches.len() {
                table.begin();
            }
        }

        Self {
            props,
            header: self.header.move_with_state(state),
            table,
            footer: self.footer.move_with_state(state),
            ..self
        }
    }

    fn name(&self) -> &str {
        "patches"
    }

    fn handle_key_event(&mut self, key: Key) {
        match key {
            Key::Up | Key::Char('k') => {
                self.table.prev();
            }
            Key::Down | Key::Char('j') => {
                self.table.next(self.props.patches.len());
            }
            Key::PageUp => {
                self.table.prev_page(self.props.page_size);
            }
            Key::PageDown => {
                self.table
                    .next_page(self.props.patches.len(), self.props.page_size);
            }
            Key::Home => {
                self.table.begin();
            }
            Key::End => {
                self.table.end(self.props.patches.len());
            }
            _ => {}
        }
        self.table
            .selected()
            .and_then(|selected| self.props.patches.get(selected))
            .and_then(|patch| {
                self.action_tx
                    .send(Action::Select {
                        item: patch.clone(),
                    })
                    .ok()
            });
    }
}

impl Patches {
    fn render_header<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect) {
        self.header.render::<B>(
            frame,
            area,
            HeaderProps {
                cells: [
                    String::from(" ● ").into(),
                    String::from("ID").into(),
                    String::from("Title").into(),
                    String::from("Author").into(),
                    String::from("").into(),
                    String::from("Head").into(),
                    String::from("+").into(),
                    String::from("- ").into(),
                    String::from("Updated").into(),
                ],
                widths: self.props.widths,
                focus: self.props.focus,
                cutoff: self.props.cutoff,
                cutoff_after: self.props.cutoff_after,
            },
        );
    }

    fn render_list<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect) {
        self.table.render::<B>(
            frame,
            area,
            TableProps {
                items: self.props.patches.to_vec(),
                has_header: true,
                has_footer: !self.props.show_search,
                widths: self.props.widths,
                focus: self.props.focus,
                cutoff: self.props.cutoff,
                cutoff_after: self.props.cutoff_after,
            },
        );
    }

    fn render_footer<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect) {
        let search = if self.props.search.read().is_empty() {
            Line::from(
                [span::default(self.props.search.read().to_string())
                    .magenta()
                    .dim()]
                .to_vec(),
            )
        } else {
            Line::from(
                [
                    span::default(" / ".to_string()).magenta().dim(),
                    span::default(self.props.search.read().to_string())
                        .magenta()
                        .dim(),
                ]
                .to_vec(),
            )
        };

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

        match PatchItemFilter::from_str(&self.props.search.read())
            .unwrap_or_default()
            .status()
        {
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
    pub input: InputBox,
}

impl Widget<PatchesState, Action> for Search {
    fn new(state: &PatchesState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        let mut input = InputBox::new(state, action_tx.clone());
        input.set_text(&state.search.read().to_string());

        Self { action_tx, input }.move_with_state(state)
    }

    fn move_with_state(self, state: &PatchesState) -> Self
    where
        Self: Sized,
    {
        let mut input =
            <InputBox as Widget<PatchesState, Action>>::move_with_state(self.input, state);
        input.set_text(&state.search.read().to_string());

        Self { input, ..self }
    }

    fn name(&self) -> &str {
        "filter-popup"
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
                <InputBox as Widget<PatchesState, Action>>::handle_key_event(&mut self.input, key);
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
            input_box::RenderProps {
                titles: ("/".into(), "Search".into()),
                show_cursor: true,
                inline_label: true,
            },
        );
    }
}
