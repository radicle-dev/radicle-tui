use std::marker::PhantomData;
use std::str::FromStr;

use radicle::issue::{self, CloseReason};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Row;
use ratatui::Frame;

use radicle_tui as tui;

use tui::store;
use tui::ui::theme::style;
use tui::ui::widget::{RenderProps, View, ViewProps};
use tui::ui::{layout, span};

use super::format;
use super::items::IssueItem;

use crate::ui::items::Filter;

/// A `BrowserState` represents the internal state of a browser widget.
/// A browser widget would consist of 2 child widgets: a list of items and a
/// buffered search field. The search fields value is used to build an
/// item filter that the item list reacts on dynamically.
#[derive(Clone, Debug)]
pub struct BrowserState<I, F> {
    items: Vec<I>,
    selected: Option<usize>,
    filter: F,
    search: store::StateValue<String>,
    show_search: bool,
}

impl<I, F> Default for BrowserState<I, F>
where
    I: Clone,
    F: Filter<I> + Default + FromStr,
{
    fn default() -> Self {
        Self {
            items: vec![],
            selected: None,
            filter: F::default(),
            search: store::StateValue::new(String::default()),
            show_search: false,
        }
    }
}

impl<I, F> BrowserState<I, F>
where
    I: Clone,
    F: Filter<I> + Default + FromStr,
{
    pub fn build(items: Vec<I>, filter: F, search: store::StateValue<String>) -> Self {
        let selected = items.first().map(|_| 0);

        Self {
            items,
            selected,
            filter,
            search,
            ..Default::default()
        }
    }

    pub fn items(&self) -> Vec<I> {
        self.items_ref().into_iter().cloned().collect()
    }

    pub fn items_ref(&self) -> Vec<&I> {
        self.items
            .iter()
            .filter(|patch| self.filter.matches(patch))
            .collect()
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn selected_item(&self) -> Option<&I> {
        self.selected
            .and_then(|selected| self.items_ref().get(selected).copied())
    }

    pub fn select_item(&mut self, selected: Option<usize>) -> Option<&I> {
        self.selected = selected;
        self.selected_item()
    }

    pub fn select_first_item(&mut self) -> Option<&I> {
        self.selected.and_then(|selected| {
            if selected > self.items_ref().len() {
                self.selected = Some(0);
                self.items_ref().first().cloned()
            } else {
                self.items_ref().get(selected).cloned()
            }
        })
    }

    fn filter_items(&mut self) {
        self.filter = F::from_str(&self.search.read()).unwrap_or_default();
    }

    pub fn update_search(&mut self, value: String) {
        self.search.write(value);
        self.filter_items();
    }

    pub fn show_search(&mut self) {
        self.show_search = true;
    }

    pub fn hide_search(&mut self) {
        self.show_search = false;
    }

    pub fn apply_search(&mut self) {
        self.search.apply();
    }

    pub fn reset_search(&mut self) {
        self.search.reset();
        self.filter_items();
    }

    pub fn is_search_shown(&self) -> bool {
        self.show_search
    }

    pub fn read_search(&self) -> String {
        self.search.read()
    }
}

#[derive(Clone, Default)]
pub struct IssueDetailsProps {
    issue: Option<IssueItem>,
    dim: bool,
}

impl IssueDetailsProps {
    pub fn issue(mut self, issue: Option<IssueItem>) -> Self {
        self.issue = issue;
        self
    }

    pub fn dim(mut self, dim: bool) -> Self {
        self.dim = dim;
        self
    }
}

pub struct IssueDetails<S, M> {
    /// Phantom
    phantom: PhantomData<(S, M)>,
}

impl<S, M> Default for IssueDetails<S, M> {
    fn default() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<S, M> View for IssueDetails<S, M> {
    type State = S;
    type Message = M;

    fn render(&mut self, props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        let default = IssueDetailsProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<IssueDetailsProps>())
            .unwrap_or(&default);

        let [area] = Layout::default()
            .constraints([Constraint::Min(1)])
            .horizontal_margin(1)
            .areas(render.area);

        if let Some(issue) = props.issue.as_ref() {
            let author = match &issue.author.alias {
                Some(alias) => {
                    if issue.author.you {
                        span::alias(&format!("{}", alias))
                    } else {
                        span::alias(alias)
                    }
                }
                None => match &issue.author.human_nid {
                    Some(nid) => span::alias(nid).dim(),
                    None => span::blank(),
                },
            };

            let did = match &issue.author.human_nid {
                Some(nid) => {
                    if issue.author.you {
                        span::alias("(you)").dim().italic()
                    } else {
                        span::alias(nid).dim()
                    }
                }
                None => span::blank(),
            };

            let labels = format::labels(&issue.labels);

            let status = match issue.state {
                issue::State::Open => Text::styled("open", style::green()),
                issue::State::Closed { reason } => match reason {
                    CloseReason::Solved => Line::from(
                        [
                            Span::styled("closed", style::red()),
                            Span::raw(" "),
                            Span::styled("(solved)", style::red().italic().dim()),
                        ]
                        .to_vec(),
                    )
                    .into(),
                    CloseReason::Other => Text::styled("closed", style::red()),
                },
            };

            let table = ratatui::widgets::Table::new(
                [
                    Row::new([
                        Text::raw("Title").cyan(),
                        Text::raw(issue.title.clone()).bold(),
                    ]),
                    Row::new([
                        Text::raw("Issue").cyan(),
                        Text::raw(issue.id.to_string()).bold(),
                    ]),
                    Row::new([
                        Text::raw("Author").cyan(),
                        Line::from([author, " ".into(), did].to_vec()).into(),
                    ]),
                    Row::new([Text::raw("Labels").cyan(), Text::from(labels).blue()]),
                    Row::new([Text::raw("Status").cyan(), status]),
                ],
                [Constraint::Length(8), Constraint::Fill(1)],
            );

            let table = if !render.focus && props.dim {
                table.dim()
            } else {
                table
            };

            frame.render_widget(table, area);
        } else {
            let center = layout::centered_rect(render.area, 50, 10);
            let hint = Text::from(span::default("No issue selected"))
                .centered()
                .light_magenta()
                .dim();

            frame.render_widget(hint, center);
        }
    }
}
