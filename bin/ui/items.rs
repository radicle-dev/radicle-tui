pub mod notification;
pub mod patch;

use std::collections::HashMap;
use std::fmt::Debug;
use std::str::FromStr;

use nom::bytes::complete::{tag, take};
use nom::multi::separated_list0;
use nom::sequence::{delimited, preceded};
use nom::{IResult, Parser};

use radicle::cob::thread::{Comment, CommentId};
use radicle::cob::{Label, ObjectId, Timestamp};

use radicle::identity::Did;
use radicle::issue;
use radicle::issue::{CloseReason, Issue, IssueId};
use radicle::node::{Alias, AliasStore, NodeId};
use radicle::Profile;

use ratatui::prelude::*;
use ratatui::style::{Style, Stylize};
use ratatui::widgets::Cell;

use tui_tree_widget::TreeItem;

use radicle_tui as tui;

use tui::ui::span;
use tui::ui::{ToRow, ToTree};

use super::format;

pub mod filter {
    use std::fmt::{self, Write};
    use std::str::FromStr;

    use nom::bytes::complete::{tag_no_case, take};
    use nom::character::complete::{char, multispace0};
    use nom::combinator::map;
    use nom::multi::separated_list1;
    use nom::sequence::delimited;
    use nom::{IResult, Parser};

    use radicle::prelude::Did;

    pub const FUZZY_MIN_SCORE: i64 = 50;

    /// A generic filter that needs be implemented for item filters in order to
    /// apply it.
    pub trait Filter<T> {
        fn matches(&self, item: &T) -> bool;
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum DidFilter {
        Single(Did),
        Or(Vec<Did>),
    }

    impl fmt::Display for DidFilter {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                DidFilter::Single(did) => write!(f, "{did}")?,
                DidFilter::Or(dids) => {
                    let mut it = dids.iter().peekable();
                    f.write_char('(')?;
                    while let Some(did) = it.next() {
                        write!(f, "{did}")?;
                        if it.peek().is_none() {
                            write!(f, " or ")?;
                        }
                    }
                    f.write_char(')')?;
                }
            }
            Ok(())
        }
    }

    pub fn parse_did_single(input: &str) -> IResult<&str, DidFilter> {
        let (input, did) = take(56_usize)(input)?;

        match Did::from_str(did) {
            Ok(did) => IResult::Ok((input, DidFilter::Single(did))),
            Err(_) => IResult::Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Verify,
            ))),
        }
    }

    pub fn parse_did_or(input: &str) -> IResult<&str, DidFilter> {
        map(
            delimited(
                (multispace0, char('('), multispace0),
                separated_list1(
                    delimited(multispace0, tag_no_case("or"), multispace0),
                    take(56_usize),
                ),
                (multispace0, char(')'), multispace0),
            ),
            |dids: Vec<&str>| {
                DidFilter::Or(
                    dids.iter()
                        .filter_map(|did| Did::from_str(did).ok())
                        .collect::<Vec<_>>(),
                )
            },
        )
        .parse(input)
    }
}

pub trait HasId {
    fn id(&self) -> ObjectId;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorItem {
    pub nid: Option<NodeId>,
    pub human_nid: Option<String>,
    pub alias: Option<Alias>,
    pub you: bool,
}

impl AuthorItem {
    pub fn new(nid: Option<NodeId>, profile: &Profile) -> Self {
        let alias = match nid {
            Some(nid) => profile.alias(&nid),
            None => None,
        };
        let you = nid.map(|nid| nid == *profile.id()).unwrap_or_default();
        let human_nid = nid.map(|nid| format::did(&Did::from(nid)));

        Self {
            nid,
            human_nid,
            alias,
            you,
        }
    }
}

#[derive(Clone, Debug)]
pub struct IssueItem {
    /// Issue OID.
    pub id: IssueId,
    /// Issue state.
    pub state: issue::State,
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

impl IssueItem {
    pub fn new(profile: &Profile, issue: (IssueId, Issue)) -> Result<Self, anyhow::Error> {
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

impl ToRow<8> for IssueItem {
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

impl HasId for IssueItem {
    fn id(&self) -> ObjectId {
        self.id
    }
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct IssueItemFilter {
    state: Option<issue::State>,
    authored: bool,
    authors: Vec<Did>,
    assigned: bool,
    assignees: Vec<Did>,
    search: Option<String>,
}

impl IssueItemFilter {
    pub fn state(&self) -> Option<issue::State> {
        self.state
    }
}

impl filter::Filter<IssueItem> for IssueItemFilter {
    fn matches(&self, issue: &IssueItem) -> bool {
        use fuzzy_matcher::skim::SkimMatcherV2;
        use fuzzy_matcher::FuzzyMatcher;

        let matcher = SkimMatcherV2::default();

        let matches_state = match self.state {
            Some(issue::State::Closed {
                reason: CloseReason::Other,
            }) => matches!(issue.state, issue::State::Closed { .. }),
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

impl FromStr for IssueItemFilter {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
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
                "is:open" => state = Some(issue::State::Open),
                "is:closed" => {
                    state = Some(issue::State::Closed {
                        reason: issue::CloseReason::Other,
                    })
                }
                "is:solved" => {
                    state = Some(issue::State::Closed {
                        reason: issue::CloseReason::Solved,
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

/// A `CommentItem` represents a comment COB and is constructed from an `Issue` and
/// a `Comment`.
#[derive(Clone, Debug)]
pub struct CommentItem {
    /// Comment OID.
    pub id: CommentId,
    /// Author of this comment.
    pub author: AuthorItem,
    /// The content of this comment.
    pub body: String,
    /// Reactions to this comment.
    pub reactions: Vec<char>,
    /// Time when patch was opened.
    pub timestamp: Timestamp,
    /// The parent OID if this is a reply.
    pub reply_to: Option<CommentId>,
    /// Replies to this comment.
    pub replies: Vec<CommentItem>,
}

impl CommentItem {
    pub fn new(profile: &Profile, issue: (IssueId, Issue), comment: (CommentId, Comment)) -> Self {
        let (issue_id, issue) = issue;
        let (comment_id, comment) = comment;

        Self {
            id: comment_id,
            author: AuthorItem::new(Some(NodeId::from(*comment.author().0)), profile),
            body: comment.body().to_string(),
            reactions: comment.reactions().iter().map(|r| r.0.emoji()).collect(),
            timestamp: comment.timestamp(),
            reply_to: comment.reply_to(),
            replies: issue
                .thread()
                .replies(&comment_id)
                .map(|(reply_id, reply)| {
                    CommentItem::new(
                        profile,
                        (issue_id, issue.clone()),
                        (*reply_id, reply.clone()),
                    )
                })
                .collect(),
        }
    }

    pub fn accumulated_reactions(&self) -> Vec<(char, usize)> {
        let mut accumulated: HashMap<char, usize> = HashMap::new();

        for reaction in &self.reactions {
            if let Some(count) = accumulated.get_mut(reaction) {
                *count = count.saturating_add(1);
            } else {
                accumulated.insert(*reaction, 1_usize);
            }
        }

        let mut sorted = accumulated.into_iter().collect::<Vec<_>>();
        sorted.sort();

        sorted
    }

    pub fn path_to(&self, target_id: &CommentId, path: &mut Vec<CommentId>) -> bool {
        path.push(self.id);

        if self.id == *target_id {
            return true;
        }

        for reply in &self.replies {
            if reply.path_to(target_id, path) {
                return true;
            }
        }
        path.pop();

        false
    }
}

impl ToTree<String> for CommentItem {
    fn rows(&self) -> Vec<TreeItem<'_, String>> {
        let children = self.replies.iter().flat_map(CommentItem::rows).collect();

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
        let action = if self.reply_to.is_none() {
            "opened"
        } else {
            "commented"
        };
        let timestamp = span::timestamp(&format::timestamp(&self.timestamp));

        let text = Text::from(Line::from(
            [author, " ".into(), action.into(), " ".into(), timestamp].to_vec(),
        ));
        let item = TreeItem::new(self.id.to_string(), text, children)
            .expect("Identifiers need to be unique");

        vec![item]
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::*;

    #[test]
    fn issue_item_filter_from_str_should_succeed() -> Result<()> {
        let search = r#"is:open is:assigned assignees:[did:key:z6MkkpTPzcq1ybmjQyQpyre15JUeMvZY6toxoZVpLZ8YarsB,did:key:z6Mku8hpprWTmCv3BqkssCYDfr2feUdyLSUnycVajFo9XVAx] is:authored authors:[did:key:z6Mku8hpprWTmCv3BqkssCYDfr2feUdyLSUnycVajFo9XVAx] cli"#;
        let actual = IssueItemFilter::from_str(search)?;

        let expected = IssueItemFilter {
            state: Some(issue::State::Open),
            authors: vec![Did::from_str(
                "did:key:z6Mku8hpprWTmCv3BqkssCYDfr2feUdyLSUnycVajFo9XVAx",
            )?],
            authored: true,
            assigned: true,
            assignees: vec![
                Did::from_str("did:key:z6MkkpTPzcq1ybmjQyQpyre15JUeMvZY6toxoZVpLZ8YarsB")?,
                Did::from_str("did:key:z6Mku8hpprWTmCv3BqkssCYDfr2feUdyLSUnycVajFo9XVAx")?,
            ],
            search: Some("cli".to_string()),
        };

        assert_eq!(expected, actual);

        Ok(())
    }
}
