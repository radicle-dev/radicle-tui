use std::collections::HashMap;
use std::str::FromStr;
use std::vec;

use radicle::issue::{self, CloseReason};
use ratatui::Frame;
use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::layout::{Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::text::{Line, Text};

use radicle_tui as tui;

use tui::ui::span;
use tui::ui::widget;
use tui::ui::widget::container::{
    Column, Container, ContainerProps, Footer, FooterProps, Header, HeaderProps,
};
use tui::ui::widget::input::{TextField, TextFieldProps};
use tui::ui::widget::list::{Table, TableProps};
use tui::ui::widget::ViewProps;
use tui::ui::widget::{RenderProps, ToWidget, View};

use tui::BoxedAny;

use crate::ui::items::{IssueItem, IssueItemFilter};

use super::{Message, State};

type Widget = widget::Widget<State, Message>;

#[derive(Clone, Default)]
pub struct BrowserProps<'a> {
    /// Filtered issues.
    issues: Vec<IssueItem>,
    /// Issue statistics.
    stats: HashMap<String, usize>,
    /// Header columns
    header: Vec<Column<'a>>,
    /// Table columns
    columns: Vec<Column<'a>>,
    /// If search widget should be shown.
    show_search: bool,
    /// Current search string.
    search: String,
}

impl<'a> From<&State> for BrowserProps<'a> {
    fn from(state: &State) -> Self {
        use radicle::issue::State;

        let issues = state.browser.issues();

        let mut open = 0;
        let mut other = 0;
        let mut solved = 0;

        for issue in &issues {
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

        let stats = HashMap::from([
            ("Open".to_string(), open),
            ("Other".to_string(), other),
            ("Solved".to_string(), solved),
            ("Closed".to_string(), closed),
        ]);

        Self {
            issues,
            stats,
            header: [
                Column::new(" ● ", Constraint::Length(3)),
                Column::new("ID", Constraint::Length(8)),
                Column::new("Title", Constraint::Fill(5)),
                Column::new("Author", Constraint::Length(16)).hide_small(),
                Column::new("", Constraint::Length(16)).hide_medium(),
                Column::new("Labels", Constraint::Fill(1)).hide_medium(),
                Column::new("Assignees", Constraint::Fill(1)).hide_medium(),
                Column::new("Opened", Constraint::Length(16)).hide_small(),
            ]
            .to_vec(),
            columns: [
                Column::new(" ● ", Constraint::Length(3)),
                Column::new("ID", Constraint::Length(8)),
                Column::new("Title", Constraint::Fill(5)),
                Column::new("Author", Constraint::Length(16)).hide_small(),
                Column::new("", Constraint::Length(16)).hide_medium(),
                Column::new("Labels", Constraint::Fill(1)).hide_medium(),
                Column::new("Assignees", Constraint::Fill(1)).hide_medium(),
                Column::new("Opened", Constraint::Length(16)).hide_small(),
            ]
            .to_vec(),
            search: state.browser.search.read(),
            show_search: state.browser.show_search,
        }
    }
}

pub struct Browser {
    /// Notifications widget
    issues: Widget,
    /// Search widget
    search: Widget,
}

impl Browser {
    pub fn new(tx: UnboundedSender<Message>) -> Self {
        Self {
            issues: Container::default()
                .header(Header::default().to_widget(tx.clone()).on_update(|state| {
                    // TODO: remove and use state directly
                    let props = BrowserProps::from(state);
                    HeaderProps::default()
                        .columns(props.header.clone())
                        .theme(state.theme.clone())
                        .to_boxed_any()
                        .into()
                }))
                .content(
                    Table::<State, Message, IssueItem, 8>::default()
                        .to_widget(tx.clone())
                        .on_event(|_, s, _| {
                            let (selected, _) =
                                s.and_then(|s| s.unwrap_table()).unwrap_or_default();
                            Some(Message::SelectIssue {
                                selected: Some(selected),
                            })
                        })
                        .on_update(|state| {
                            let props = BrowserProps::from(state);

                            TableProps::default()
                                .columns(props.columns)
                                .items(state.browser.issues())
                                .selected(state.browser.selected)
                                .to_boxed_any()
                                .into()
                        }),
                )
                .footer(Footer::default().to_widget(tx.clone()).on_update(|state| {
                    let props = BrowserProps::from(state);

                    FooterProps::default()
                        .columns(browse_footer(&props))
                        .theme(state.theme.clone())
                        .to_boxed_any()
                        .into()
                }))
                .to_widget(tx.clone())
                .on_update(|state| {
                    ContainerProps::default()
                        .hide_footer(BrowserProps::from(state).show_search)
                        .theme(state.theme.clone())
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
                Key::Char('/') => Some(Message::OpenSearch),
                _ => {
                    self.issues.handle_event(key);
                    None
                }
            }
        }
    }

    fn update(&mut self, _props: Option<&ViewProps>, state: &Self::State) {
        self.issues.update(state);
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

            self.issues.render(RenderProps::from(table_area), frame);
            self.search
                .render(RenderProps::from(search_area).focus(render.focus), frame);
        } else {
            self.issues.render(render, frame);
        }
    }
}

fn browse_footer<'a>(props: &BrowserProps<'a>) -> Vec<Column<'a>> {
    let search = Line::from(vec![
        span::default(" Search ").cyan().dim().reversed(),
        span::default(" "),
        span::default(&props.search).gray().dim(),
    ]);

    let open = Line::from(vec![
        span::positive(&props.stats.get("Open").unwrap_or(&0).to_string()).dim(),
        span::default(" Open").dim(),
    ]);
    let solved = Line::from(vec![
        span::default(&props.stats.get("Solved").unwrap_or(&0).to_string())
            .magenta()
            .dim(),
        span::default(" Solved").dim(),
    ]);
    let closed = Line::from(vec![
        span::default(&props.stats.get("Closed").unwrap_or(&0).to_string())
            .magenta()
            .dim(),
        span::default(" Closed").dim(),
    ]);
    let sum = Line::from(vec![
        span::default("Σ ").dim(),
        span::default(&props.issues.len().to_string()).dim(),
    ]);

    match IssueItemFilter::from_str(&props.search)
        .unwrap_or_default()
        .state()
    {
        Some(state) => {
            let block = match state {
                issue::State::Open => open,
                issue::State::Closed {
                    reason: issue::CloseReason::Other,
                } => closed,
                issue::State::Closed {
                    reason: issue::CloseReason::Solved,
                } => solved,
            };

            [
                Column::new(Text::from(search), Constraint::Fill(1)),
                Column::new(
                    Text::from(block.clone()),
                    Constraint::Min(block.width() as u16),
                ),
                Column::new(Text::from(sum.clone()), Constraint::Min(sum.width() as u16)),
            ]
            .to_vec()
        }
        None => [
            Column::new(Text::from(search), Constraint::Fill(1)),
            Column::new(
                Text::from(open.clone()),
                Constraint::Min(open.width() as u16),
            ),
            Column::new(
                Text::from(closed.clone()),
                Constraint::Min(closed.width() as u16),
            ),
            Column::new(Text::from(sum.clone()), Constraint::Min(sum.width() as u16)),
        ]
        .to_vec(),
    }
}
