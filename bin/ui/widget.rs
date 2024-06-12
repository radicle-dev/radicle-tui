use std::marker::PhantomData;

use ratatui::layout::Constraint;
use ratatui::style::Stylize;
use ratatui::text::{Line, Text};
use ratatui::widgets::Row;
use ratatui::Frame;

use radicle_tui as tui;

use tui::ui::span;
use tui::ui::widget::{RenderProps, View, ViewProps};

use super::items::IssueItem;

#[derive(Clone, Default)]
pub struct IssueDetailsProps {
    issue: Option<IssueItem>,
}

impl IssueDetailsProps {
    pub fn issue(mut self, issue: Option<IssueItem>) -> Self {
        self.issue = issue;
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

        if let Some(issue) = props.issue.as_ref() {
            let author = match &issue.author.alias {
                Some(alias) => {
                    if issue.author.you {
                        span::alias(&format!("{} (you)", alias))
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
                Some(nid) => span::alias(nid).dim(),
                None => span::blank(),
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
                    Row::new([Text::raw("Status").cyan(), Text::raw("???").magenta()]),
                ],
                [Constraint::Length(8), Constraint::Fill(1)],
            );
            frame.render_widget(table, render.area);
        }
    }
}
