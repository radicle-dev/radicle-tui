use std::fmt::Debug;

use radicle::cob::thread::CommentId;
use radicle::cob::{Label, ObjectId, Timestamp};
use radicle::issue::IssueId;

use radicle::Profile;

use ratatui::style::{Style, Stylize};
use ratatui::widgets::Cell;

use radicle_tui as tui;

use tui::ui::span;
use tui::ui::ToRow;

use crate::ui::format;
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

pub mod filter {
    use std::fmt;
    use std::fmt::Debug;
    use std::fmt::Write as _;
    use std::str::FromStr;

    use nom::branch::alt;
    use nom::bytes::complete::{tag_no_case, take_while1};
    use nom::character::complete::multispace0;
    use nom::combinator::{map, value};
    use nom::multi::many0;
    use nom::sequence::preceded;
    use nom::IResult;

    use radicle::issue::CloseReason;
    use radicle::issue::State;

    use crate::ui::items::filter;
    use crate::ui::items::filter::DidFilter;
    use crate::ui::items::filter::Filter;

    use super::Issue;

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum IssueFilter {
        State(State),
        Author(DidFilter),
        Assignee(DidFilter),
        Search(String),
        And(Vec<IssueFilter>),
        Empty,
        Invalid,
    }

    impl Default for IssueFilter {
        fn default() -> Self {
            IssueFilter::State(State::Open)
        }
    }

    impl IssueFilter {
        pub fn is_default(&self) -> bool {
            *self == IssueFilter::default()
        }

        pub fn has_state(&self) -> bool {
            match self {
                IssueFilter::State(_) => true,
                IssueFilter::And(filters) => {
                    filters.iter().any(|f| matches!(f, IssueFilter::State(_)))
                }
                _ => false,
            }
        }
    }

    impl fmt::Display for IssueFilter {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                IssueFilter::State(state) => {
                    let state = match state {
                        State::Open => "open",
                        State::Closed { reason } => match reason {
                            CloseReason::Solved => "solved",
                            CloseReason::Other => "closed",
                        },
                    };
                    write!(f, "state={state}")?;
                    f.write_char(' ')?;
                }
                IssueFilter::Author(filter) => {
                    write!(f, "author={filter}")?;
                    f.write_char(' ')?;
                }
                IssueFilter::Assignee(filter) => {
                    write!(f, "assignee={filter}")?;
                    f.write_char(' ')?;
                }
                IssueFilter::Search(search) => {
                    write!(f, "{search}")?;
                    f.write_char(' ')?;
                }
                IssueFilter::And(filters) => {
                    let mut it = filters.iter().peekable();
                    while let Some(filter) = it.next() {
                        write!(f, "{filter}")?;
                        if it.peek().is_none() {
                            f.write_char(' ')?;
                        }
                    }
                }
                IssueFilter::Empty | IssueFilter::Invalid => {}
            }

            Ok(())
        }
    }

    impl Filter<Issue> for IssueFilter {
        fn matches(&self, issue: &Issue) -> bool {
            use fuzzy_matcher::skim::SkimMatcherV2;
            use fuzzy_matcher::FuzzyMatcher;

            let matcher = SkimMatcherV2::default();

            match self {
                IssueFilter::State(state) => issue.state == *state,
                IssueFilter::Author(author_filter) => match author_filter {
                    DidFilter::Single(author) => issue.author.nid == Some(**author),
                    DidFilter::Or(authors) => authors
                        .iter()
                        .any(|other| issue.author.nid == Some(**other)),
                },
                IssueFilter::Assignee(assignee_filter) => match assignee_filter {
                    DidFilter::Single(assignee) => issue
                        .assignees
                        .iter()
                        .any(|other| other.nid == Some(**assignee)),
                    DidFilter::Or(assignees) => issue.assignees.iter().any(|other| {
                        assignees
                            .iter()
                            .any(|assignee| other.nid == Some(**assignee))
                    }),
                },
                IssueFilter::Search(search) => {
                    match matcher.fuzzy_match(
                        &format!(
                            "{} {} {}",
                            &issue.id.to_string(),
                            &issue.title,
                            &issue
                                .author
                                .alias
                                .as_ref()
                                .map(|a| a.to_string())
                                .unwrap_or_default()
                        ),
                        search,
                    ) {
                        Some(score) => score == 0 || score > filter::FUZZY_MIN_SCORE,
                        _ => false,
                    }
                }
                IssueFilter::And(filters) => filters.iter().all(|f| f.matches(issue)),
                IssueFilter::Empty => true,
                IssueFilter::Invalid => false,
            }
        }
    }

    impl FromStr for IssueFilter {
        type Err = anyhow::Error;

        fn from_str(filter_exp: &str) -> Result<Self, Self::Err> {
            use nom::Parser;

            fn parse_state(input: &str) -> IResult<&str, State> {
                alt((
                    value(State::Open, tag_no_case("open")),
                    value(
                        State::Closed {
                            reason: radicle::issue::CloseReason::Other,
                        },
                        tag_no_case("closed"),
                    ),
                    value(
                        State::Closed {
                            reason: radicle::issue::CloseReason::Solved,
                        },
                        tag_no_case("solved"),
                    ),
                ))
                .parse(input)
            }

            fn parse_state_filter(input: &str) -> IResult<&str, IssueFilter> {
                map(
                    preceded(
                        (
                            tag_no_case("state"),
                            multispace0,
                            tag_no_case("="),
                            multispace0,
                        ),
                        parse_state,
                    ),
                    IssueFilter::State,
                )
                .parse(input)
            }

            fn parse_assignee_filter(input: &str) -> IResult<&str, IssueFilter> {
                map(
                    preceded(
                        (
                            tag_no_case("assignee"),
                            multispace0,
                            tag_no_case("="),
                            multispace0,
                        ),
                        alt((filter::parse_did_single, filter::parse_did_or)),
                    ),
                    IssueFilter::Assignee,
                )
                .parse(input)
            }

            fn parse_author_filter(input: &str) -> IResult<&str, IssueFilter> {
                map(
                    preceded(
                        (
                            tag_no_case("author"),
                            multispace0,
                            tag_no_case("="),
                            multispace0,
                        ),
                        alt((filter::parse_did_single, filter::parse_did_or)),
                    ),
                    IssueFilter::Author,
                )
                .parse(input)
            }

            fn parse_search_filter(input: &str) -> IResult<&str, IssueFilter> {
                map(
                    take_while1(|c: char| c.is_alphanumeric() || c == '_' || c == '-'),
                    |s: &str| IssueFilter::Search(s.to_string()),
                )
                .parse(input)
            }

            fn parse_single_filter(input: &str) -> IResult<&str, IssueFilter> {
                alt((
                    parse_state_filter,
                    parse_assignee_filter,
                    parse_author_filter,
                    parse_search_filter,
                ))
                .parse(input)
            }

            fn parse_filters(input: &str) -> IResult<&str, Vec<IssueFilter>> {
                many0(preceded(multispace0, parse_single_filter)).parse(input)
            }

            let parse_filter_expression = |input: &str| -> Result<IssueFilter, String> {
                match parse_filters(input) {
                    Ok((remaining, filters)) => {
                        let remaining = remaining.trim();
                        if !remaining.is_empty() {
                            return Err(format!("Unparsed input remaining: '{remaining}'"));
                        }

                        if filters.is_empty() {
                            return Ok(IssueFilter::Empty);
                        }

                        if filters.len() == 1 {
                            Ok(filters.into_iter().next().unwrap())
                        } else {
                            Ok(IssueFilter::And(filters))
                        }
                    }
                    Err(e) => Err(format!("Parse error: {e}")),
                }
            };

            parse_filter_expression(filter_exp).map_err(|err| anyhow::format_err!(err))
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::str::FromStr;

    use radicle::{issue::State, prelude::Did};

    use crate::ui::items::{filter::DidFilter, issue::filter::IssueFilter};

    #[test]
    fn issue_item_filter_from_str_should_succeed() -> Result<()> {
        let search = r#"state=open assignee=(did:key:z6MkkpTPzcq1ybmjQyQpyre15JUeMvZY6toxoZVpLZ8YarsB or did:key:z6Mku8hpprWTmCv3BqkssCYDfr2feUdyLSUnycVajFo9XVAx) author=did:key:z6Mku8hpprWTmCv3BqkssCYDfr2feUdyLSUnycVajFo9XVAx cli"#;
        let actual = IssueFilter::from_str(search)?;

        let expected = IssueFilter::And(vec![
            IssueFilter::State(State::Open),
            IssueFilter::Assignee(DidFilter::Or(vec![
                Did::from_str("did:key:z6MkkpTPzcq1ybmjQyQpyre15JUeMvZY6toxoZVpLZ8YarsB")?,
                Did::from_str("did:key:z6Mku8hpprWTmCv3BqkssCYDfr2feUdyLSUnycVajFo9XVAx")?,
            ])),
            IssueFilter::Author(DidFilter::Single(Did::from_str(
                "did:key:z6Mku8hpprWTmCv3BqkssCYDfr2feUdyLSUnycVajFo9XVAx",
            )?)),
            IssueFilter::Search("cli".to_string()),
        ]);

        assert_eq!(expected, actual);

        Ok(())
    }
}
