use std::collections::HashMap;
use std::str::FromStr;
use std::vec;

use ratatui::Frame;
use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::layout::{Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::text::{Line, Text};

use radicle::patch;
use radicle::patch::Status;

use radicle_tui as tui;

use tui::ui::span;
use tui::ui::widget;
use tui::ui::widget::container::{
    Column, Container, ContainerProps, Footer, FooterProps, Header, HeaderProps,
};
use tui::ui::widget::input::{TextField, TextFieldProps};
use tui::ui::widget::list::{Table, TableProps};
use tui::ui::widget::utils;
use tui::ui::widget::ViewProps;
use tui::ui::widget::{RenderProps, ToWidget, View};

use tui::{BoxedAny, Selection};

use crate::tui_patch::common::Mode;
use crate::tui_patch::common::PatchOperation;
use crate::ui::items::{PatchItem, PatchItemFilter};

use super::{Message, State};

type Widget = widget::Widget<State, Message>;

#[derive(Clone, Default)]
pub struct BrowserProps<'a> {
    /// Application mode: openation and id or id only.
    mode: Mode,
    /// Filtered patches.
    patches: Vec<PatchItem>,
    /// Current (selected) table index
    selected: Option<usize>,
    /// Patch statistics.
    stats: HashMap<String, usize>,
    /// Header columns
    header: Vec<Column<'a>>,
    /// Table columns
    columns: Vec<Column<'a>>,
    /// Current page size (height of table content).
    page_size: usize,
    /// If search widget should be shown.
    show_search: bool,
    /// Current search string.
    search: String,
}

impl<'a> From<&State> for BrowserProps<'a> {
    fn from(state: &State) -> Self {
        let mut draft = 0;
        let mut open = 0;
        let mut archived = 0;
        let mut merged = 0;

        let patches = state.browser.patches();

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
            selected: state.browser.selected,
            stats,
            header: [
                Column::new(" ● ", Constraint::Length(3)),
                Column::new("ID", Constraint::Length(8)),
                Column::new("Title", Constraint::Fill(1)),
                Column::new("Author", Constraint::Length(16)).hide_small(),
                Column::new("", Constraint::Length(16)).hide_medium(),
                Column::new("Head", Constraint::Length(8)).hide_small(),
                Column::new("+", Constraint::Length(6)).hide_small(),
                Column::new("-", Constraint::Length(6)).hide_small(),
                Column::new("Updated", Constraint::Length(16)).hide_small(),
            ]
            .to_vec(),
            columns: [
                Column::new(" ● ", Constraint::Length(3)),
                Column::new("ID", Constraint::Length(8)),
                Column::new("Title", Constraint::Fill(1)),
                Column::new("Author", Constraint::Length(16)).hide_small(),
                Column::new("", Constraint::Length(16)).hide_medium(),
                Column::new("Head", Constraint::Length(8)).hide_small(),
                Column::new("+", Constraint::Length(6)).hide_small(),
                Column::new("-", Constraint::Length(6)).hide_small(),
                Column::new("Updated", Constraint::Length(16)).hide_small(),
            ]
            .to_vec(),
            page_size: state.browser.page_size,
            show_search: state.browser.show_search,
            search: state.browser.search.read(),
        }
    }
}

pub struct Browser {
    /// Patches widget
    patches: Widget,
    /// Search widget
    search: Widget,
}

impl Browser {
    pub fn new(tx: UnboundedSender<Message>) -> Self {
        Self {
            patches: Container::default()
                .header(Header::default().to_widget(tx.clone()).on_update(|state| {
                    // TODO: remove and use state directly
                    let props = BrowserProps::from(state);
                    HeaderProps::default()
                        .columns(props.header.clone())
                        .to_boxed_any()
                        .into()
                }))
                .content(
                    Table::<State, Message, PatchItem, 9>::default()
                        .to_widget(tx.clone())
                        .on_event(|_, s, _| {
                            Some(Message::Select {
                                selected: s.and_then(|s| s.unwrap_usize()),
                            })
                        })
                        .on_update(|state| {
                            // TODO: remove and use state directly
                            let props = BrowserProps::from(state);
                            TableProps::default()
                                .columns(props.columns)
                                .items(state.browser.patches())
                                .selected(state.browser.selected)
                                .footer(!state.browser.show_search)
                                .page_size(state.browser.page_size)
                                .to_boxed_any()
                                .into()
                        }),
                )
                .footer(Footer::default().to_widget(tx.clone()).on_update(|state| {
                    // TODO: remove and use state directly
                    let props = BrowserProps::from(state);

                    FooterProps::default()
                        .columns(browser_footer(&props, props.selected))
                        .to_boxed_any()
                        .into()
                }))
                .to_widget(tx.clone())
                .on_update(|state| {
                    ContainerProps::default()
                        .hide_footer(BrowserProps::from(state).show_search)
                        .to_boxed_any()
                        .into()
                }),
            search: TextField::default()
                .to_widget(tx.clone())
                .on_event(|_, s, _| {
                    Some(Message::UpdateSearch {
                        value: s.and_then(|i| i.unwrap_string()).unwrap_or_default(),
                    })
                })
                .on_update(|state: &State| {
                    TextFieldProps::default()
                        .text(&state.browser.search.read().to_string())
                        .title("Search")
                        .inline(true)
                        .to_boxed_any()
                        .into()
                }),
        }
    }
}

impl View for Browser {
    type Message = Message;
    type State = State;

    fn handle_event(&mut self, props: Option<&ViewProps>, key: Key) -> Option<Self::Message> {
        let default = BrowserProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<BrowserProps>())
            .unwrap_or(&default);

        if props.show_search {
            match key {
                Key::Esc => {
                    self.search.reset();
                    Some(Message::CloseSearch)
                }
                Key::Char('\n') => Some(Message::ApplySearch),
                _ => {
                    self.search.handle_event(key);
                    None
                }
            }
        } else {
            match key {
                Key::Esc | Key::Ctrl('c') => Some(Message::Exit { selection: None }),
                Key::Char('?') => Some(Message::OpenHelp),
                Key::Char('/') => Some(Message::OpenSearch),
                Key::Char('\n') => {
                    let operation = match props.mode {
                        Mode::Operation => Some(PatchOperation::Show.to_string()),
                        Mode::Id => None,
                    };

                    props
                        .selected
                        .and_then(|selected| props.patches.get(selected))
                        .map(|patch| Message::Exit {
                            selection: Some(Selection {
                                operation,
                                ids: vec![patch.id],
                                args: vec![],
                            }),
                        })
                }
                Key::Char('c') => props
                    .selected
                    .and_then(|selected| props.patches.get(selected))
                    .map(|patch| Message::Exit {
                        selection: Some(Selection {
                            operation: Some(PatchOperation::Checkout.to_string()),
                            ids: vec![patch.id],
                            args: vec![],
                        }),
                    }),
                Key::Char('d') => props
                    .selected
                    .and_then(|selected| props.patches.get(selected))
                    .map(|patch| Message::Exit {
                        selection: Some(Selection {
                            operation: Some(PatchOperation::Diff.to_string()),
                            ids: vec![patch.id],
                            args: vec![],
                        }),
                    }),
                _ => {
                    self.patches.handle_event(key);
                    None
                }
            }
        }
    }

    fn update(&mut self, _props: Option<&ViewProps>, state: &Self::State) {
        self.patches.update(state);
        self.search.update(state);
    }

    fn render(&mut self, props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let default = BrowserProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<BrowserProps>())
            .unwrap_or(&default);

        if props.show_search {
            let [table_area, search_area] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(render.area);
            let [_, search_area, _] = Layout::horizontal([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .areas(search_area);

            self.patches.render(RenderProps::from(table_area), frame);
            self.search
                .render(RenderProps::from(search_area).focus(render.focus), frame);
        } else {
            self.patches.render(render, frame);
        }
    }
}

fn browser_footer<'a>(props: &BrowserProps<'a>, selected: Option<usize>) -> Vec<Column<'a>> {
    let filter = PatchItemFilter::from_str(&props.search).unwrap_or_default();

    let search = Line::from(vec![
        span::default(" Search ").cyan().dim().reversed(),
        span::default(" "),
        span::default(&props.search.to_string()).gray().dim(),
    ]);

    let draft = Line::from(vec![
        span::default(&props.stats.get("Draft").unwrap_or(&0).to_string()).dim(),
        span::default(" Draft").dim(),
    ]);

    let open = Line::from(vec![
        span::positive(&props.stats.get("Open").unwrap_or(&0).to_string()).dim(),
        span::default(" Open").dim(),
    ]);

    let merged = Line::from(vec![
        span::default(&props.stats.get("Merged").unwrap_or(&0).to_string())
            .magenta()
            .dim(),
        span::default(" Merged").dim(),
    ]);

    let archived = Line::from(vec![
        span::default(&props.stats.get("Archived").unwrap_or(&0).to_string())
            .yellow()
            .dim(),
        span::default(" Archived").dim(),
    ]);

    let sum = Line::from(vec![
        span::default("Σ ").dim(),
        span::default(&props.patches.len().to_string()).dim(),
    ]);

    let progress = selected
        .map(|selected| {
            utils::scroll::percent_absolute(
                selected.saturating_sub(props.page_size),
                props.patches.len(),
                props.page_size,
            )
        })
        .unwrap_or_default();
    let progress = span::default(&format!("{}%", progress)).dim();

    match filter.status() {
        Some(state) => {
            let block = match state {
                Status::Draft => draft,
                Status::Open => open,
                Status::Merged => merged,
                Status::Archived => archived,
            };

            vec![
                Column::new(Text::from(search), Constraint::Fill(1)),
                Column::new(
                    Text::from(block.clone()),
                    Constraint::Min(block.width() as u16),
                ),
                Column::new(Text::from(progress), Constraint::Min(4)),
            ]
        }
        None => vec![
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
        ],
    }
}
