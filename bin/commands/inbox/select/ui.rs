use std::collections::HashMap;
use std::str::FromStr;

use ratatui::Frame;
use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::layout::{Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::text::{Line, Text};

use radicle_tui as tui;

use tui::ui::rm::widget::container::{
    Column, Container, ContainerProps, Footer, FooterProps, Header, HeaderProps,
};
use tui::ui::rm::widget::input::{TextField, TextFieldProps};
use tui::ui::rm::widget::list::{Table, TableProps};
use tui::ui::rm::widget::{self, ViewProps};
use tui::ui::rm::widget::{RenderProps, ToWidget, View};
use tui::ui::span;

use tui::{BoxedAny, Selection};

use crate::tui_inbox::common::{InboxOperation, Mode, RepositoryMode, SelectionMode};
use crate::ui::items::{NotificationItem, NotificationItemFilter, NotificationState};

use super::{Message, State};

type Widget = widget::Widget<State, Message>;

#[derive(Clone, Default)]
pub struct BrowserProps<'a> {
    /// Application mode: openation and id or id only.
    mode: Mode,
    /// Table title
    header: String,
    /// Filtered notifications.
    notifications: Vec<NotificationItem>,
    /// Current (selected) table index
    selected: Option<usize>,
    /// Notification statistics.
    stats: HashMap<String, usize>,
    /// Table columns
    columns: Vec<Column<'a>>,
    /// If search widget should be shown.
    show_search: bool,
    /// Current search string.
    search: String,
}

impl<'a> From<&State> for BrowserProps<'a> {
    fn from(state: &State) -> Self {
        let header = match state.mode.repository() {
            RepositoryMode::Contextual => state.project.name().to_string(),
            RepositoryMode::All => "All repositories".to_string(),
            RepositoryMode::ByRepo((_, name)) => name.clone().unwrap_or_default(),
        };

        let notifications = state.browser.notifications();

        // Compute statistics
        let mut seen = 0;
        let mut unseen = 0;
        for notification in &notifications {
            if notification.seen {
                seen += 1;
            } else {
                unseen += 1;
            }
        }
        let stats = HashMap::from([("Seen".to_string(), seen), ("Unseen".to_string(), unseen)]);

        Self {
            mode: state.mode.clone(),
            header,
            notifications,
            selected: state.browser.selected,
            stats,
            columns: [
                Column::new("", Constraint::Length(5)),
                Column::new("", Constraint::Length(3)),
                Column::new("", Constraint::Fill(5)),
                Column::new("", Constraint::Fill(1))
                    .skip(*state.mode.repository() != RepositoryMode::All),
                Column::new("", Constraint::Fill(1))
                    .hide_small()
                    .hide_medium(),
                Column::new("", Constraint::Length(8)),
                Column::new("", Constraint::Length(10)),
                Column::new("", Constraint::Min(12)).hide_small(),
                Column::new("", Constraint::Min(14)).hide_small(),
            ]
            .to_vec(),
            search: state.browser.search.read(),
            show_search: state.browser.show_search,
        }
    }
}

pub struct Browser {
    /// Notification widget
    notifications: Widget,
    /// Search widget
    search: Widget,
}

impl Browser {
    pub fn new(tx: UnboundedSender<Message>) -> Self {
        Self {
            notifications: Container::default()
                .header(Header::default().to_widget(tx.clone()).on_update(|state| {
                    // TODO: remove and use state directly
                    let props = BrowserProps::from(state);
                    HeaderProps::default()
                        .columns(
                            [
                                Column::new("", Constraint::Length(0)),
                                Column::new(Text::from(props.header), Constraint::Fill(1)),
                            ]
                            .to_vec(),
                        )
                        .to_boxed_any()
                        .into()
                }))
                .content(
                    Table::<State, Message, NotificationItem, 9>::default()
                        .to_widget(tx.clone())
                        .on_event(|_, s, _| {
                            let (selected, _) =
                                s.and_then(|s| s.unwrap_table()).unwrap_or_default();
                            Some(Message::Select {
                                selected: Some(selected),
                            })
                        })
                        .on_update(|state| {
                            let props = BrowserProps::from(state);

                            TableProps::default()
                                .columns(props.columns)
                                .items(state.browser.notifications())
                                .selected(state.browser.selected)
                                .to_boxed_any()
                                .into()
                        }),
                )
                .footer(Footer::default().to_widget(tx.clone()).on_update(|state| {
                    let props = BrowserProps::from(state);

                    FooterProps::default()
                        .columns(browse_footer(&props))
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
                Key::Char('/') => Some(Message::OpenSearch),
                Key::Char('\n') => props
                    .selected
                    .and_then(|selected| props.notifications.get(selected))
                    .map(|notif| {
                        let selection = match props.mode.selection() {
                            SelectionMode::Operation => Selection::default()
                                .with_operation(InboxOperation::Show.to_string())
                                .with_id(notif.id),
                            SelectionMode::Id => Selection::default().with_id(notif.id),
                        };

                        Message::Exit {
                            selection: Some(selection),
                        }
                    }),
                Key::Char('c') => props
                    .selected
                    .and_then(|selected| props.notifications.get(selected))
                    .map(|notif| Message::Exit {
                        selection: Some(
                            Selection::default()
                                .with_operation(InboxOperation::Clear.to_string())
                                .with_id(notif.id),
                        ),
                    }),
                _ => {
                    self.notifications.handle_event(key);
                    None
                }
            }
        }
    }

    fn update(&mut self, _props: Option<&ViewProps>, state: &Self::State) {
        self.notifications.update(state);
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

            self.notifications
                .render(RenderProps::from(table_area), frame);
            self.search
                .render(RenderProps::from(search_area).focus(render.focus), frame);
        } else {
            self.notifications.render(render, frame);
        }
    }
}

fn browse_footer<'a>(props: &BrowserProps<'a>) -> Vec<Column<'a>> {
    let search = Line::from(vec![
        span::default(" Search ").cyan().dim().reversed(),
        span::default(" "),
        span::default(&props.search.to_string()).gray().dim(),
    ]);

    let seen = Line::from(vec![
        span::positive(&props.stats.get("Seen").unwrap_or(&0).to_string()).dim(),
        span::default(" Seen").dim(),
    ]);
    let unseen = Line::from(vec![
        span::positive(&props.stats.get("Unseen").unwrap_or(&0).to_string())
            .magenta()
            .dim(),
        span::default(" Unseen").dim(),
    ]);
    let sum = Line::from(vec![
        span::default("Σ ").dim(),
        span::default(&props.notifications.len().to_string()).dim(),
    ]);

    match NotificationItemFilter::from_str(&props.search)
        .unwrap_or_default()
        .state()
    {
        Some(state) => {
            let block = match state {
                NotificationState::Seen => seen,
                NotificationState::Unseen => unseen,
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
                Text::from(seen.clone()),
                Constraint::Min(seen.width() as u16),
            ),
            Column::new(
                Text::from(unseen.clone()),
                Constraint::Min(unseen.width() as u16),
            ),
            Column::new(Text::from(sum.clone()), Constraint::Min(sum.width() as u16)),
        ]
        .to_vec(),
    }
}
