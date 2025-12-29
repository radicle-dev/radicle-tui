use std::fmt::Debug;
use std::str::FromStr;

use nom::bytes::complete::{tag, take};
use nom::multi::separated_list0;
use nom::sequence::{delimited, preceded};
use nom::{IResult, Parser};

use radicle::cob::thread::CommentId;
use radicle::cob::{Label, ObjectId, Timestamp};
use radicle::issue::{CloseReason, IssueId};
use radicle::prelude::Did;
use radicle::Profile;

use ratatui::style::{Style, Stylize};
use ratatui::widgets::Cell;

use radicle_tui as tui;

use tui::ui::span;
use tui::ui::ToRow;

use crate::ui::format;
use crate::ui::items::filter::Filter;
use crate::ui::items::{AuthorItem, CommentItem, HasId};

#[derive(Clone, Debug)]
pub struct Issue {
    /// Issue OID.
    pub id: IssueId,
    /// Issue state.
    pub state: radicle::issue::State,
    /// Issue title.
    pub title: String,
    /// Issue author.
    pub author: AuthorItem,
    /// Issue labels.
    pub labels: Vec<Label>,
    /// Issue assignees.
    pub assignees: Vec<AuthorItem>,
    /// Time when issue was opened.
    pub timestamp: Timestamp,
    /// Comment timeline
    pub comments: Vec<CommentItem>,
}

impl Issue {
    pub fn new(
        profile: &Profile,
        issue: (IssueId, radicle::issue::Issue),
    ) -> Result<Self, anyhow::Error> {
        let (id, issue) = issue;

        Ok(Self {
            id,
            state: *issue.state(),
            title: issue.title().into(),
            author: AuthorItem::new(Some(*issue.author().id), profile),
            labels: issue.labels().cloned().collect(),
            assignees: issue
                .assignees()
                .map(|did| AuthorItem::new(Some(**did), profile))
                .collect::<Vec<_>>(),
            timestamp: issue.timestamp(),
            comments: issue
                .comments()
                .map(|(comment_id, comment)| {
                    CommentItem::new(profile, (id, issue.clone()), (*comment_id, comment.clone()))
                })
                .collect(),
        })
    }

    pub fn root_comments(&self) -> Vec<CommentItem> {
        self.comments
            .iter()
            .filter(|comment| comment.reply_to.is_none())
            .cloned()
            .collect::<Vec<_>>()
    }

    pub fn has_comment(&self, comment_id: &CommentId) -> bool {
        self.comments
            .iter()
            .any(|comment| comment.id == *comment_id)
    }

    pub fn path_to_comment(&self, comment_id: &CommentId) -> Option<Vec<CommentId>> {
        for comment in &self.comments {
            let mut path = Vec::new();
            if comment.path_to(comment_id, &mut path) {
                return Some(path);
            }
        }
        None
    }
}

impl ToRow<8> for Issue {
    fn to_row(&self) -> [Cell<'_>; 8] {
        let (state, state_color) = format::issue_state(&self.state);

        let state = span::default(&state).style(Style::default().fg(state_color));
        let id = span::primary(&format::cob(&self.id));
        let title = span::default(&self.title.clone());

        let author = match &self.author.alias {
            Some(alias) => {
                if self.author.you {
                    span::alias(&format!("{alias} (you)"))
                } else {
                    span::alias(alias)
                }
            }
            None => match &self.author.human_nid {
                Some(nid) => span::alias(nid).dim(),
                None => span::blank(),
            },
        };
        let did = match &self.author.human_nid {
            Some(nid) => span::alias(nid).dim(),
            None => span::blank(),
        };
        let labels = span::labels(&format::labels(&self.labels));
        let assignees = self
            .assignees
            .iter()
            .map(|author| (author.nid, author.alias.clone(), author.you))
            .collect::<Vec<_>>();
        let assignees = span::alias(&format::assignees(&assignees));
        let opened = span::timestamp(&format::timestamp(&self.timestamp));

        [
            state.into(),
            id.into(),
            title.into(),
            author.into(),
            did.into(),
            labels.into(),
            assignees.into(),
            opened.into(),
        ]
    }
}

impl HasId for Issue {
    fn id(&self) -> ObjectId {
        self.id
    }
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub(crate) struct IssueFilter {
    pub(crate) state: Option<radicle::issue::State>,
    pub(crate) authored: bool,
    pub(crate) authors: Vec<Did>,
    pub(crate) assigned: bool,
    pub(crate) assignees: Vec<Did>,
    pub(crate) search: Option<String>,
}

impl IssueFilter {
    pub fn state(&self) -> Option<radicle::issue::State> {
        self.state
    }
}

impl Filter<Issue> for IssueFilter {
    fn matches(&self, issue: &Issue) -> bool {
        use fuzzy_matcher::skim::SkimMatcherV2;
        use fuzzy_matcher::FuzzyMatcher;
        use radicle::issue::State;

        let matcher = SkimMatcherV2::default();

        let matches_state = match self.state {
            Some(State::Closed {
                reason: CloseReason::Other,
            }) => matches!(issue.state, State::Closed { .. }),
            Some(state) => issue.state == state,
            None => true,
        };

        let matches_authored = if self.authored {
            issue.author.you
        } else {
            true
        };

        let matches_authors = if !self.authors.is_empty() {
            {
                self.authors
                    .iter()
                    .any(|other| issue.author.nid == Some(**other))
            }
        } else {
            true
        };

        let matches_assigned = if self.assigned {
            issue.assignees.iter().any(|assignee| assignee.you)
        } else {
            true
        };

        let matches_assignees = if !self.assignees.is_empty() {
            {
                self.assignees.iter().any(|other| {
                    issue
                        .assignees
                        .iter()
                        .filter_map(|author| author.nid)
                        .collect::<Vec<_>>()
                        .contains(other)
                })
            }
        } else {
            true
        };

        let matches_search = match &self.search {
            Some(search) => match matcher.fuzzy_match(&issue.title, search) {
                Some(score) => score == 0 || score > 60,
                _ => false,
            },
            None => true,
        };

        matches_state
            && matches_authored
            && matches_authors
            && matches_assigned
            && matches_assignees
            && matches_search
    }
}

impl FromStr for IssueFilter {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        use radicle::issue::State;

        let mut state = None;
        let mut search = String::new();
        let mut authored = false;
        let mut authors = vec![];
        let mut assigned = false;
        let mut assignees = vec![];

        let mut authors_parser = |input| -> IResult<&str, Vec<&str>> {
            preceded(
                tag("authors:"),
                delimited(
                    tag("["),
                    separated_list0(tag(","), take(56_usize)),
                    tag("]"),
                ),
            )
            .parse(input)
        };

        let mut assignees_parser = |input| -> IResult<&str, Vec<&str>> {
            preceded(
                tag("assignees:"),
                delimited(
                    tag("["),
                    separated_list0(tag(","), take(56_usize)),
                    tag("]"),
                ),
            )
            .parse(input)
        };

        let parts = value.split(' ');
        for part in parts {
            match part {
                "is:open" => state = Some(State::Open),
                "is:closed" => {
                    state = Some(State::Closed {
                        reason: CloseReason::Other,
                    })
                }
                "is:solved" => {
                    state = Some(State::Closed {
                        reason: CloseReason::Solved,
                    })
                }
                "is:authored" => authored = true,
                "is:assigned" => assigned = true,
                other => {
                    if let Ok((_, dids)) = assignees_parser.parse(other) {
                        for did in dids {
                            assignees.push(Did::from_str(did)?);
                        }
                    } else if let Ok((_, dids)) = authors_parser.parse(other) {
                        for did in dids {
                            authors.push(Did::from_str(did)?);
                        }
                    } else {
                        search.push_str(other);
                    }
                }
            }
        }

        Ok(Self {
            state,
            authored,
            authors,
            assigned,
            assignees,
            search: Some(search),
        })
    }
}
