use std::fmt;

use radicle::cob::{ObjectId, Timestamp, TypeName, TypedId};
use radicle::identity::Identity;
use radicle::node;
use radicle::prelude::Project;
use radicle::storage::git::Repository;
use radicle::storage::{ReadRepository, RefUpdate};
use radicle::Profile;

use ratatui::style::Stylize;
use ratatui::widgets::Cell;

use radicle_tui as tui;

use tui::ui::span;
use tui::ui::theme::style;
use tui::ui::ToRow;

use super::AuthorItem;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotificationState {
    Seen,
    Unseen,
}

impl fmt::Display for NotificationState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotificationState::Seen => write!(f, "seen")?,
            NotificationState::Unseen => write!(f, "unseen")?,
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum NotificationKind {
    Cob {
        type_name: Option<TypeName>,
        summary: Option<String>,
        status: Option<String>,
        id: Option<ObjectId>,
    },
    Branch {
        name: Option<String>,
        summary: Option<String>,
        status: Option<String>,
        _id: Option<ObjectId>,
    },
    Unknown {
        refname: Option<String>,
    },
}

impl PartialEq for NotificationKind {
    fn eq(&self, other: &Self) -> bool {
        match self {
            NotificationKind::Cob { type_name, .. } => {
                let other_type_name = match other {
                    NotificationKind::Cob { type_name, .. } => type_name.clone(),
                    _ => None,
                };
                other_type_name.is_some() && (other_type_name == *type_name)
            }
            NotificationKind::Branch { name, .. } => {
                let other_name = match other {
                    NotificationKind::Branch { name, .. } => name.clone(),
                    _ => None,
                };
                other_name.is_some() && (other_name == *name)
            }
            NotificationKind::Unknown { refname } => {
                let other_refname = match other {
                    NotificationKind::Unknown { refname, .. } => refname.clone(),
                    _ => None,
                };
                other_refname.is_some() && (other_refname == *refname)
            }
        }
    }
}

impl fmt::Display for NotificationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotificationKind::Cob { type_name, .. } => {
                write!(f, "cob")?;
                if let Some(type_name) = type_name {
                    write!(f, ":{type_name}")?;
                }
            }
            NotificationKind::Branch { name, .. } => {
                write!(f, "branch")?;
                if let Some(name) = name {
                    write!(f, ":{name}")?;
                }
            }
            NotificationKind::Unknown { refname, .. } => {
                write!(f, "unknown")?;
                if let Some(refname) = refname {
                    write!(f, ":{refname}")?;
                }
            }
        }
        Ok(())
    }
}

impl NotificationKind {
    pub fn new<I, P>(
        repo: &Repository,
        issues: &I,
        patches: &P,
        notification: &node::notifications::Notification,
    ) -> Result<Option<Self>, anyhow::Error>
    where
        I: radicle::cob::issue::cache::Issues,
        P: radicle::cob::patch::cache::Patches,
    {
        match &notification.kind {
            node::notifications::NotificationKind::Branch { name } => {
                let (head, message) = if let Some(head) = notification.update.new() {
                    let message = repo.commit(head)?.summary().unwrap_or_default().to_owned();
                    (Some(head), message)
                } else {
                    (None, String::new())
                };
                let status = match notification
                    .update
                    .new()
                    .map(|oid| repo.is_ancestor_of(oid, head.unwrap()))
                    .transpose()
                {
                    Ok(Some(true)) => "merged",
                    Ok(Some(false)) | Ok(None) => match notification.update {
                        RefUpdate::Updated { .. } => "updated",
                        RefUpdate::Created { .. } => "created",
                        RefUpdate::Deleted { .. } => "deleted",
                        RefUpdate::Skipped { .. } => "skipped",
                    },
                    Err(e) => return Err(e.into()),
                }
                .to_owned();

                Ok(Some(NotificationKind::Branch {
                    name: Some(name.to_string()),
                    summary: Some(message),
                    status: Some(status.to_string()),
                    _id: head.map(ObjectId::from),
                }))
            }
            node::notifications::NotificationKind::Cob { typed_id } => {
                let TypedId { id, type_name } = typed_id;
                let (type_name, summary, state) = if typed_id.is_issue() {
                    let Some(issue) = issues.get(id)? else {
                        // Issue could have been deleted after notification was created.
                        return Ok(None);
                    };
                    (
                        type_name,
                        issue.title().to_string(),
                        issue.state().to_string(),
                    )
                } else if typed_id.is_patch() {
                    let Some(patch) = patches.get(id)? else {
                        // Patch could have been deleted after notification was created.
                        return Ok(None);
                    };
                    (
                        type_name,
                        patch.title().to_string(),
                        patch.state().to_string(),
                    )
                } else if typed_id.is_identity() {
                    let Ok(identity) = Identity::get(id, repo) else {
                        log::error!(
                            target: "items",
                            "Error retrieving identity {id} for notification {}", notification.id
                        );
                        return Ok(None);
                    };
                    let Some(rev) = notification
                        .update
                        .new()
                        .and_then(|id| identity.revision(&id))
                    else {
                        log::error!(
                            target: "items",
                            "Error retrieving identity revision for notification {}", notification.id
                        );
                        return Ok(None);
                    };
                    (type_name, rev.title.to_string(), rev.state.to_string())
                } else {
                    (type_name, "".to_string(), "".to_string())
                };

                Ok(Some(NotificationKind::Cob {
                    type_name: Some(type_name.clone()),
                    summary: Some(summary.to_string()),
                    status: Some(state.to_string()),
                    id: Some(*id),
                }))
            }
            node::notifications::NotificationKind::Unknown { refname } => {
                Ok(Some(NotificationKind::Unknown {
                    refname: Some(refname.to_string()),
                }))
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Notification {
    /// Unique notification ID.
    pub id: node::notifications::NotificationId,
    /// The project this belongs to.
    pub project: String,
    /// Mark this notification as seen.
    pub seen: bool,
    /// Wrapped notification kind.
    pub kind: NotificationKind,
    /// The author
    pub author: AuthorItem,
    /// Time the update has happened.
    pub timestamp: Timestamp,
}

impl Notification {
    pub fn new(
        profile: &Profile,
        project: &Project,
        notification: &node::notifications::Notification,
        kind: NotificationKind,
    ) -> Result<Option<Self>, anyhow::Error> {
        Ok(Some(Notification {
            id: notification.id,
            project: project.name().to_string(),
            seen: notification.status.is_read(),
            kind,
            author: AuthorItem::new(notification.remote, profile),
            timestamp: notification.timestamp.into(),
        }))
    }
}

impl ToRow<9> for Notification {
    fn to_row(&self) -> [Cell<'_>; 9] {
        let (type_name, summary, status, kind_id) = match &self.kind {
            NotificationKind::Branch {
                name,
                summary,
                status,
                _id: _,
            } => (
                Some("branch".to_string()),
                summary.clone(),
                status.clone(),
                name.clone(),
            ),
            NotificationKind::Cob {
                type_name,
                summary,
                status,
                id,
            } => {
                let id = id.map(|id| super::format::cob(&id)).unwrap_or_default();
                (
                    type_name.as_ref().map(|t| t.to_string()),
                    summary.clone(),
                    status.clone(),
                    Some(id.to_string()),
                )
            }
            NotificationKind::Unknown { refname } => (refname.clone(), None, None, None),
        };

        let id = span::notification_id(&format!(" {:-03}", &self.id));
        let seen = if self.seen {
            span::blank()
        } else {
            span::primary(" ● ")
        };
        let kind_id = span::primary(&kind_id.unwrap_or_default());
        let summary = span::default(&summary.unwrap_or_default());
        let type_name = span::notification_type(&type_name.unwrap_or_default());
        let name = span::default(&self.project.clone()).style(style::gray().dim());

        let status = status.unwrap_or_default();
        let status = match status.as_str() {
            "archived" => span::default(&status).yellow(),
            "draft" => span::default(&status).gray().dim(),
            "updated" => span::primary(&status),
            "open" | "created" => span::positive(&status),
            "closed" | "merged" => span::ternary(&status),
            _ => span::default(&status),
        };
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
        let timestamp = span::timestamp(&super::format::timestamp(&self.timestamp));

        [
            seen.into(),
            id.into(),
            summary.into(),
            name.into(),
            kind_id.into(),
            type_name.into(),
            status.into(),
            author.into(),
            timestamp.into(),
        ]
    }
}

pub mod filter {
    use std::fmt;
    use std::fmt::Write as _;
    use std::str::FromStr;

    use nom::branch::alt;
    use nom::bytes::complete::{tag, tag_no_case, take_while1};
    use nom::character::complete::{char, multispace0};
    use nom::combinator::{map, opt, value};
    use nom::multi::{many0, separated_list1};
    use nom::sequence::{delimited, preceded};
    use nom::IResult;
    use radicle::cob::TypeName;

    use crate::ui::items::filter;
    use crate::ui::items::filter::{DidFilter, Filter};

    use super::{Notification, NotificationKind, NotificationState};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct SortBy {
        pub reverse: bool,
        pub field: &'static str,
    }

    impl Default for SortBy {
        fn default() -> Self {
            Self {
                reverse: true,
                field: "timestamp",
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum NotificationFilter {
        State(NotificationState),
        Kind(NotificationKindFilter),
        Author(DidFilter),
        Search(String),
        And(Vec<NotificationFilter>),
        Empty,
        Invalid,
    }

    impl Default for NotificationFilter {
        fn default() -> Self {
            NotificationFilter::And(vec![
                NotificationFilter::Kind(NotificationKindFilter::Or(vec![
                    NotificationKind::Cob {
                        type_name: TypeName::from_str("xyz.radicle.patch").ok(),
                        summary: None,
                        status: None,
                        id: None,
                    },
                    NotificationKind::Cob {
                        type_name: TypeName::from_str("xyz.radicle.issue").ok(),
                        summary: None,
                        status: None,
                        id: None,
                    },
                ])),
                NotificationFilter::State(NotificationState::Unseen),
            ])
        }
    }

    impl NotificationFilter {
        pub fn is_default(&self) -> bool {
            *self == NotificationFilter::default()
        }
    }

    impl fmt::Display for NotificationFilter {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                NotificationFilter::State(state) => {
                    write!(f, "state={state}")?;
                    f.write_char(' ')?;
                }
                NotificationFilter::Kind(filter) => {
                    write!(f, "kind={filter}")?;
                    f.write_char(' ')?;
                }
                NotificationFilter::Author(filter) => {
                    write!(f, "author={filter}")?;
                    f.write_char(' ')?;
                }
                NotificationFilter::Search(search) => {
                    write!(f, "{search}")?;
                    f.write_char(' ')?;
                }
                NotificationFilter::And(filters) => {
                    let mut it = filters.iter().peekable();
                    while let Some(filter) = it.next() {
                        write!(f, "{filter}")?;
                        if it.peek().is_none() {
                            f.write_char(' ')?;
                        }
                    }
                }
                NotificationFilter::Empty | NotificationFilter::Invalid => {}
            }

            Ok(())
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum NotificationKindFilter {
        Single(NotificationKind),
        Or(Vec<NotificationKind>),
    }

    impl fmt::Display for NotificationKindFilter {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                NotificationKindFilter::Single(type_name) => write!(f, "{type_name}")?,
                NotificationKindFilter::Or(type_names) => {
                    let mut it = type_names.iter().peekable();
                    f.write_char('(')?;
                    while let Some(type_name) = it.next() {
                        write!(f, "{type_name}")?;
                        if it.peek().is_some() {
                            write!(f, " or ")?;
                        }
                    }
                    f.write_char(')')?;
                }
            }
            Ok(())
        }
    }

    impl Filter<Notification> for NotificationFilter {
        fn matches(&self, notif: &Notification) -> bool {
            use fuzzy_matcher::skim::SkimMatcherV2;
            use fuzzy_matcher::FuzzyMatcher;

            let matcher = SkimMatcherV2::default();

            match self {
                NotificationFilter::State(state) => match state {
                    NotificationState::Seen => notif.seen,
                    NotificationState::Unseen => !notif.seen,
                },
                NotificationFilter::Kind(type_filter) => match type_filter {
                    NotificationKindFilter::Single(kind) => &notif.kind == kind,
                    NotificationKindFilter::Or(kinds) => {
                        kinds.iter().any(|kind| &notif.kind == kind)
                    }
                },
                NotificationFilter::Author(author_filter) => match author_filter {
                    DidFilter::Single(author) => notif.author.nid == Some(**author),
                    DidFilter::Or(authors) => authors
                        .iter()
                        .any(|other| notif.author.nid == Some(**other)),
                },
                NotificationFilter::Search(search) => {
                    let summary = match &notif.kind {
                        NotificationKind::Cob {
                            type_name: _,
                            summary,
                            status: _,
                            id: _,
                        } => summary.clone().unwrap_or_default(),
                        NotificationKind::Branch {
                            name: _,
                            summary,
                            status: _,
                            _id: _,
                        } => summary.clone().unwrap_or_default(),
                        NotificationKind::Unknown { refname } => {
                            refname.clone().unwrap_or_default()
                        }
                    };
                    match matcher.fuzzy_match(&summary, search) {
                        Some(score) => score == 0 || score > 60,
                        _ => false,
                    }
                }
                NotificationFilter::And(filters) => filters.iter().all(|f| f.matches(notif)),
                NotificationFilter::Empty => true,
                NotificationFilter::Invalid => false,
            }
        }
    }

    impl FromStr for NotificationFilter {
        type Err = anyhow::Error;

        fn from_str(filter_exp: &str) -> Result<Self, Self::Err> {
            use nom::Parser;

            fn parse_state(input: &str) -> IResult<&str, NotificationState> {
                alt((
                    value(NotificationState::Seen, tag_no_case("seen")),
                    value(NotificationState::Unseen, tag_no_case("unseen")),
                ))
                .parse(input)
            }

            fn parse_name(input: &str) -> IResult<&str, &str> {
                take_while1(|c: char| {
                    c.is_alphanumeric() || c == '.' || c == '-' || c == '_' || c == '/'
                })(input)
            }

            fn parse_type_name(input: &str) -> IResult<&str, TypeName> {
                let (input, type_name) =
                    take_while1(|c: char| c.is_alphanumeric() || c == '.')(input)?;

                match TypeName::from_str(type_name) {
                    Ok(t) => IResult::Ok((input, t)),
                    Err(_) => IResult::Err(nom::Err::Error(nom::error::Error::new(
                        input,
                        nom::error::ErrorKind::Verify,
                    ))),
                }
            }

            fn parse_state_filter(input: &str) -> IResult<&str, NotificationFilter> {
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
                    NotificationFilter::State,
                )
                .parse(input)
            }

            fn parse_cob_kind(input: &str) -> IResult<&str, NotificationKind> {
                let (input, _) = tag("cob")(input)?;
                let (input, type_name) = opt(preceded(tag(":"), parse_type_name)).parse(input)?;

                Ok((
                    input,
                    NotificationKind::Cob {
                        type_name,
                        summary: None,
                        status: None,
                        id: None,
                    },
                ))
            }

            fn parse_branch_kind(input: &str) -> IResult<&str, NotificationKind> {
                let (input, _) = tag("branch")(input)?;
                let (input, name) = opt(preceded(tag(":"), parse_name)).parse(input)?;

                Ok((
                    input,
                    NotificationKind::Branch {
                        name: name.map(|n| n.to_string()),
                        summary: None,
                        status: None,
                        _id: None,
                    },
                ))
            }

            fn parse_unknown_kind(input: &str) -> IResult<&str, NotificationKind> {
                let (input, _) = tag("unknown")(input)?;
                let (input, refname) = opt(preceded(tag(":"), parse_name)).parse(input)?;

                Ok((
                    input,
                    NotificationKind::Unknown {
                        refname: refname.map(|r| r.to_string()),
                    },
                ))
            }

            fn parse_kind(input: &str) -> IResult<&str, NotificationKind> {
                alt((parse_cob_kind, parse_branch_kind, parse_unknown_kind)).parse(input)
            }

            fn parse_kind_single(input: &str) -> IResult<&str, NotificationKindFilter> {
                map(parse_kind, NotificationKindFilter::Single).parse(input)
            }

            fn parse_kind_or(input: &str) -> IResult<&str, NotificationKindFilter> {
                map(
                    delimited(
                        (multispace0, char('('), multispace0),
                        separated_list1(
                            delimited(multispace0, tag_no_case("or"), multispace0),
                            parse_kind,
                        ),
                        (multispace0, char(')'), multispace0),
                    ),
                    NotificationKindFilter::Or,
                )
                .parse(input)
            }

            fn parse_kind_filter(input: &str) -> IResult<&str, NotificationFilter> {
                map(
                    preceded(
                        (
                            tag_no_case("kind"),
                            multispace0,
                            tag_no_case("="),
                            multispace0,
                        ),
                        alt((parse_kind_or, parse_kind_single)),
                    ),
                    NotificationFilter::Kind,
                )
                .parse(input)
            }

            fn parse_author_filter(input: &str) -> IResult<&str, NotificationFilter> {
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
                    NotificationFilter::Author,
                )
                .parse(input)
            }

            fn parse_search_filter(input: &str) -> IResult<&str, NotificationFilter> {
                map(
                    take_while1(|c: char| c.is_alphanumeric() || c == '_' || c == '-'),
                    |s: &str| NotificationFilter::Search(s.to_string()),
                )
                .parse(input)
            }

            fn parse_single_filter(input: &str) -> IResult<&str, NotificationFilter> {
                alt((
                    parse_state_filter,
                    parse_kind_filter,
                    parse_author_filter,
                    parse_search_filter,
                ))
                .parse(input)
            }

            fn parse_filters(input: &str) -> IResult<&str, Vec<NotificationFilter>> {
                many0(preceded(multispace0, parse_single_filter)).parse(input)
            }

            let parse_filter_expression = |input: &str| -> Result<NotificationFilter, String> {
                match parse_filters(input) {
                    Ok((remaining, filters)) => {
                        let remaining = remaining.trim();
                        if !remaining.is_empty() {
                            return Err(format!("Unparsed input remaining: '{remaining}'"));
                        }

                        if filters.is_empty() {
                            return Ok(NotificationFilter::Empty);
                        }

                        if filters.len() == 1 {
                            Ok(filters.into_iter().next().unwrap())
                        } else {
                            Ok(NotificationFilter::And(filters))
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
    use std::str::FromStr;

    use anyhow::Result;
    use radicle::prelude::Did;

    use crate::ui::items::filter::DidFilter;

    use super::filter::*;
    use super::*;

    #[test]
    fn notification_item_filter_with_concrete_kind_should_succeed() -> Result<()> {
        let search = r#"kind=cob:xyz.radicle.patch"#;
        let actual = NotificationFilter::from_str(search)?;

        let expected =
            NotificationFilter::Kind(NotificationKindFilter::Single(NotificationKind::Cob {
                type_name: TypeName::from_str("xyz.radicle.patch").ok(),
                summary: None,
                status: None,
                id: None,
            }));

        assert_eq!(actual, expected);

        Ok(())
    }

    #[test]
    fn notification_item_filter_with_author_should_succeed() -> Result<()> {
        let search = r#"author=did:key:z6MkkpTPzcq1ybmjQyQpyre15JUeMvZY6toxoZVpLZ8YarsB"#;
        let actual = NotificationFilter::from_str(search)?;

        let expected = NotificationFilter::Author(DidFilter::Single(Did::from_str(
            "did:key:z6MkkpTPzcq1ybmjQyQpyre15JUeMvZY6toxoZVpLZ8YarsB",
        )?));

        assert_eq!(expected, actual);

        Ok(())
    }

    #[test]
    fn notification_item_filter_with_author_should_not_succeed() -> Result<()> {
        let search = r#"author=foo"#;
        let result = NotificationFilter::from_str(search);

        println!("{result:?}");

        assert!(matches!(result.unwrap_err(), anyhow::Error { .. }));

        Ok(())
    }

    #[test]
    fn notification_item_filter_with_all_should_succeed() -> Result<()> {
        let search = r#"state=seen kind=(cob:xyz.radicle.patch or cob:xyz.radicle.issue) author=(did:key:z6MkkpTPzcq1ybmjQyQpyre15JUeMvZY6toxoZVpLZ8YarsB or did:key:z6Mku8hpprWTmCv3BqkssCYDfr2feUdyLSUnycVajFo9XVAx) cli"#;
        let actual = NotificationFilter::from_str(search)?;

        let expected = NotificationFilter::And(vec![
            NotificationFilter::State(NotificationState::Seen),
            NotificationFilter::Kind(NotificationKindFilter::Or(vec![
                NotificationKind::Cob {
                    type_name: TypeName::from_str("xyz.radicle.patch").ok(),
                    summary: None,
                    status: None,
                    id: None,
                },
                NotificationKind::Cob {
                    type_name: TypeName::from_str("xyz.radicle.issue").ok(),
                    summary: None,
                    status: None,
                    id: None,
                },
            ])),
            NotificationFilter::Author(DidFilter::Or(vec![
                Did::from_str("did:key:z6MkkpTPzcq1ybmjQyQpyre15JUeMvZY6toxoZVpLZ8YarsB")?,
                Did::from_str("did:key:z6Mku8hpprWTmCv3BqkssCYDfr2feUdyLSUnycVajFo9XVAx")?,
            ])),
            NotificationFilter::Search("cli".to_string()),
        ]);

        assert_eq!(expected, actual);

        Ok(())
    }

    #[test]
    fn notification_item_filter_with_all_shuffled_should_succeed() -> Result<()> {
        let search = r#"kind=(cob:xyz.radicle.patch or cob:xyz.radicle.issue) author=did:key:z6Mku8hpprWTmCv3BqkssCYDfr2feUdyLSUnycVajFo9XVAx state=seen cli"#;
        let actual = NotificationFilter::from_str(search)?;

        let expected = NotificationFilter::And(vec![
            NotificationFilter::Kind(NotificationKindFilter::Or(vec![
                NotificationKind::Cob {
                    type_name: TypeName::from_str("xyz.radicle.patch").ok(),
                    summary: None,
                    status: None,
                    id: None,
                },
                NotificationKind::Cob {
                    type_name: TypeName::from_str("xyz.radicle.issue").ok(),
                    summary: None,
                    status: None,
                    id: None,
                },
            ])),
            NotificationFilter::Author(DidFilter::Single(Did::from_str(
                "did:key:z6Mku8hpprWTmCv3BqkssCYDfr2feUdyLSUnycVajFo9XVAx",
            )?)),
            NotificationFilter::State(NotificationState::Seen),
            NotificationFilter::Search("cli".to_string()),
        ]);

        assert_eq!(expected, actual);

        Ok(())
    }
}
