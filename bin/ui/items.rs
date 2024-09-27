use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use nom::bytes::complete::{tag, take};
use nom::multi::separated_list0;
use nom::sequence::{delimited, preceded};
use nom::{IResult, Parser};

use ansi_to_tui::IntoText;

use radicle::cob::thread::{Comment, CommentId};
use radicle::cob::{Label, ObjectId, Timestamp, TypedId};
use radicle::git::Oid;
use radicle::identity::{Did, Identity};
use radicle::issue;
use radicle::issue::{CloseReason, Issue, IssueId, Issues};
use radicle::node::notifications::{Notification, NotificationId, NotificationKind};
use radicle::node::{Alias, AliasStore, NodeId};
use radicle::patch;
use radicle::patch::{Patch, PatchId, Patches};
use radicle::storage::git::Repository;
use radicle::storage::{ReadRepository, ReadStorage, RefUpdate, WriteRepository};
use radicle::Profile;

use radicle_surf::diff::{self, Hunk, Modification};

use radicle_cli::git::unified_diff::{Decode, HunkHeader};
use radicle_cli::terminal;
use radicle_cli::terminal::highlight::Highlighter;

use ratatui::style::{Color, Style, Stylize};
// use ratatui::text::{Line, Text};
use ratatui::prelude::*;
use ratatui::widgets::Cell;

use tui_tree_widget::TreeItem;

use radicle_tui as tui;

use tui::ui::span;
use tui::ui::theme::style;
use tui::ui::{ToRow, ToTree};

use crate::cob::IndexedReviewItem;
use crate::git::{Blob, Repo};

use super::super::git;
use super::format;

pub trait Filter<T> {
    fn matches(&self, item: &T) -> bool;
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
#[allow(dead_code)]
pub enum NotificationKindItem {
    Branch {
        name: String,
        summary: String,
        status: String,
        id: Option<ObjectId>,
    },
    Cob {
        type_name: String,
        summary: String,
        status: String,
        id: Option<ObjectId>,
    },
    Unknown {
        refname: String,
    },
}

impl NotificationKindItem {
    pub fn new(
        repo: &Repository,
        notification: &Notification,
    ) -> Result<Option<Self>, anyhow::Error> {
        // TODO: move out of here
        let issues = Issues::open(repo)?;
        let patches = Patches::open(repo)?;

        match &notification.kind {
            NotificationKind::Branch { name } => {
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

                Ok(Some(NotificationKindItem::Branch {
                    name: name.to_string(),
                    summary: message,
                    status: status.to_string(),
                    id: head.map(ObjectId::from),
                }))
            }
            NotificationKind::Cob { typed_id } => {
                let TypedId { id, .. } = typed_id;
                let (category, summary, state) = if typed_id.is_issue() {
                    let Some(issue) = issues.get(id)? else {
                        // Issue could have been deleted after notification was created.
                        return Ok(None);
                    };
                    (
                        String::from("issue"),
                        issue.title().to_owned(),
                        issue.state().to_string(),
                    )
                } else if typed_id.is_patch() {
                    let Some(patch) = patches.get(id)? else {
                        // Patch could have been deleted after notification was created.
                        return Ok(None);
                    };
                    (
                        String::from("patch"),
                        patch.title().to_owned(),
                        patch.state().to_string(),
                    )
                } else if typed_id.is_identity() {
                    let Ok(identity) = Identity::get(id, repo) else {
                        log::error!(
                            target: "cli",
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
                            target: "cli",
                            "Error retrieving identity revision for notification {}", notification.id
                        );
                        return Ok(None);
                    };
                    (String::from("id"), rev.title.clone(), rev.state.to_string())
                } else {
                    (typed_id.type_name.to_string(), "".to_owned(), String::new())
                };

                Ok(Some(NotificationKindItem::Cob {
                    type_name: category.to_string(),
                    summary: summary.to_string(),
                    status: state.to_string(),
                    id: Some(*id),
                }))
            }
            NotificationKind::Unknown { refname } => Ok(Some(NotificationKindItem::Unknown {
                refname: refname.to_string(),
            })),
        }
    }
}

#[derive(Clone, Debug)]
pub struct NotificationItem {
    /// Unique notification ID.
    pub id: NotificationId,
    /// The project this belongs to.
    pub project: String,
    /// Mark this notification as seen.
    pub seen: bool,
    /// Wrapped notification kind.
    pub kind: NotificationKindItem,
    /// The author
    pub author: AuthorItem,
    /// Time the update has happened.
    pub timestamp: Timestamp,
}

impl NotificationItem {
    pub fn new(
        profile: &Profile,
        repo: &Repository,
        notification: &Notification,
    ) -> Result<Option<Self>, anyhow::Error> {
        let project = profile
            .storage
            .repository(repo.id)?
            .identity_doc()?
            .project()?;
        let name = project.name().to_string();
        let kind = NotificationKindItem::new(repo, notification)?;

        if kind.is_none() {
            return Ok(None);
        }

        Ok(Some(NotificationItem {
            id: notification.id,
            project: name,
            seen: notification.status.is_read(),
            kind: kind.unwrap(),
            author: AuthorItem::new(notification.remote, profile),
            timestamp: notification.timestamp.into(),
        }))
    }
}

impl ToRow<9> for NotificationItem {
    fn to_row(&self) -> [Cell; 9] {
        let (type_name, summary, status, kind_id) = match &self.kind {
            NotificationKindItem::Branch {
                name,
                summary,
                status,
                id: _,
            } => (
                "branch".to_string(),
                summary.clone(),
                status.clone(),
                name.to_string(),
            ),
            NotificationKindItem::Cob {
                type_name,
                summary,
                status,
                id,
            } => {
                let id = id.map(|id| format::cob(&id)).unwrap_or_default();
                (
                    type_name.to_string(),
                    summary.clone(),
                    status.clone(),
                    id.to_string(),
                )
            }
            NotificationKindItem::Unknown { refname } => (
                refname.to_string(),
                String::new(),
                String::new(),
                String::new(),
            ),
        };

        let id = span::notification_id(&format!(" {:-03}", &self.id));
        let seen = if self.seen {
            span::blank()
        } else {
            span::primary(" ● ")
        };
        let kind_id = span::primary(&kind_id);
        let summary = span::default(&summary);
        let type_name = span::notification_type(&type_name);
        let name = span::default(&self.project.clone()).style(style::gray().dim());

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
                    span::alias(&format!("{} (you)", alias))
                } else {
                    span::alias(alias)
                }
            }
            None => match &self.author.human_nid {
                Some(nid) => span::alias(nid).dim(),
                None => span::blank(),
            },
        };
        let timestamp = span::timestamp(&format::timestamp(&self.timestamp));

        [
            id.into(),
            seen.into(),
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotificationType {
    Patch,
    Issue,
    Branch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotificationState {
    Seen,
    Unseen,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct NotificationItemFilter {
    state: Option<NotificationState>,
    type_name: Option<NotificationType>,
    authors: Vec<Did>,
    search: Option<String>,
}

impl NotificationItemFilter {
    pub fn state(&self) -> Option<NotificationState> {
        self.state.clone()
    }
}

impl Filter<NotificationItem> for NotificationItemFilter {
    fn matches(&self, notif: &NotificationItem) -> bool {
        use fuzzy_matcher::skim::SkimMatcherV2;
        use fuzzy_matcher::FuzzyMatcher;

        let matcher = SkimMatcherV2::default();

        let matches_state = match self.state {
            Some(NotificationState::Seen) => notif.seen,
            Some(NotificationState::Unseen) => !notif.seen,
            None => true,
        };

        let matches_type = match self.type_name {
            Some(NotificationType::Patch) => matches!(&notif.kind, NotificationKindItem::Cob {
                type_name,
                summary: _,
                status: _,
                id: _,
            } if type_name == "patch"),
            Some(NotificationType::Issue) => matches!(&notif.kind, NotificationKindItem::Cob {
                    type_name,
                    summary: _,
                    status: _,
                    id: _,
                } if type_name == "issue"),
            Some(NotificationType::Branch) => {
                matches!(notif.kind, NotificationKindItem::Branch { .. })
            }
            None => true,
        };

        let matches_authors = (!self.authors.is_empty())
            .then(|| {
                self.authors
                    .iter()
                    .any(|other| notif.author.nid == Some(**other))
            })
            .unwrap_or(true);

        let matches_search = match &self.search {
            Some(search) => {
                let summary = match &notif.kind {
                    NotificationKindItem::Cob {
                        type_name: _,
                        summary,
                        status: _,
                        id: _,
                    } => summary,
                    NotificationKindItem::Branch {
                        name: _,
                        summary,
                        status: _,
                        id: _,
                    } => summary,
                    NotificationKindItem::Unknown { refname: _ } => "",
                };
                match matcher.fuzzy_match(summary, search) {
                    Some(score) => score == 0 || score > 60,
                    _ => false,
                }
            }
            None => true,
        };

        matches_state && matches_type && matches_authors && matches_search
    }
}

impl FromStr for NotificationItemFilter {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut state = None;
        let mut type_name = None;
        let mut search = String::new();
        let mut authors = vec![];

        let mut authors_parser = |input| -> IResult<&str, Vec<&str>> {
            preceded(
                tag("authors:"),
                delimited(
                    tag("["),
                    separated_list0(tag(","), take(56_usize)),
                    tag("]"),
                ),
            )(input)
        };

        let parts = value.split(' ');
        for part in parts {
            match part {
                "is:seen" => state = Some(NotificationState::Seen),
                "is:unseen" => state = Some(NotificationState::Unseen),
                "is:patch" => type_name = Some(NotificationType::Patch),
                "is:issue" => type_name = Some(NotificationType::Issue),
                "is:branch" => type_name = Some(NotificationType::Branch),
                other => {
                    if let Ok((_, dids)) = authors_parser.parse(other) {
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
            type_name,
            authors,
            search: Some(search),
        })
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
}

impl ToRow<8> for IssueItem {
    fn to_row(&self) -> [Cell; 8] {
        let (state, state_color) = format::issue_state(&self.state);

        let state = span::default(&state).style(Style::default().fg(state_color));
        let id = span::primary(&format::cob(&self.id));
        let title = span::default(&self.title.clone());

        let author = match &self.author.alias {
            Some(alias) => {
                if self.author.you {
                    span::alias(&format!("{} (you)", alias))
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

impl Filter<IssueItem> for IssueItemFilter {
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

        let matches_authors = (!self.authors.is_empty())
            .then(|| {
                self.authors
                    .iter()
                    .any(|other| issue.author.nid == Some(**other))
            })
            .unwrap_or(true);

        let matches_assigned = self
            .assigned
            .then(|| issue.assignees.iter().any(|assignee| assignee.you))
            .unwrap_or(true);

        let matches_assignees = (!self.assignees.is_empty())
            .then(|| {
                self.assignees.iter().any(|other| {
                    issue
                        .assignees
                        .iter()
                        .filter_map(|author| author.nid)
                        .collect::<Vec<_>>()
                        .contains(other)
                })
            })
            .unwrap_or(true);

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
            )(input)
        };

        let mut assignees_parser = |input| -> IResult<&str, Vec<&str>> {
            preceded(
                tag("assignees:"),
                delimited(
                    tag("["),
                    separated_list0(tag(","), take(56_usize)),
                    tag("]"),
                ),
            )(input)
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

#[derive(Clone, Debug)]
pub struct PatchItem {
    /// Patch OID.
    pub id: PatchId,
    /// Patch state.
    pub state: patch::State,
    /// Patch title.
    pub title: String,
    /// Author of the latest revision.
    pub author: AuthorItem,
    /// Head of the latest revision.
    pub head: Oid,
    /// Lines added by the latest revision.
    pub added: u16,
    /// Lines removed by the latest revision.
    pub removed: u16,
    /// Time when patch was opened.
    pub timestamp: Timestamp,
}

impl PatchItem {
    pub fn new(
        profile: &Profile,
        repository: &Repository,
        patch: (PatchId, Patch),
    ) -> Result<Self, anyhow::Error> {
        let (id, patch) = patch;
        let (_, revision) = patch.latest();
        let (from, to) = revision.range();
        let stats = git::diff_stats(repository.raw(), &from, &to)?;

        Ok(Self {
            id,
            state: patch.state().clone(),
            title: patch.title().into(),
            author: AuthorItem::new(Some(*patch.author().id), profile),
            head: revision.head(),
            added: stats.insertions() as u16,
            removed: stats.deletions() as u16,
            timestamp: patch.updated_at(),
        })
    }
}

impl ToRow<9> for PatchItem {
    fn to_row(&self) -> [Cell; 9] {
        let (state, color) = format::patch_state(&self.state);

        let state = span::default(&state).style(Style::default().fg(color));
        let id = span::primary(&format::cob(&self.id));
        let title = span::default(&self.title.clone());

        let author = match &self.author.alias {
            Some(alias) => {
                if self.author.you {
                    span::alias(&format!("{} (you)", alias))
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

        let head = span::ternary(&format::oid(self.head));
        let added = span::positive(&format!("+{}", self.added));
        let removed = span::negative(&format!("-{}", self.removed));
        let updated = span::timestamp(&format::timestamp(&self.timestamp));

        [
            state.into(),
            id.into(),
            title.into(),
            author.into(),
            did.into(),
            head.into(),
            added.into(),
            removed.into(),
            updated.into(),
        ]
    }
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct PatchItemFilter {
    status: Option<patch::Status>,
    authored: bool,
    authors: Vec<Did>,
    search: Option<String>,
}

impl PatchItemFilter {
    pub fn status(&self) -> Option<patch::Status> {
        self.status
    }

    pub fn is_default(&self) -> bool {
        *self == PatchItemFilter::default()
    }
}

impl Filter<PatchItem> for PatchItemFilter {
    fn matches(&self, patch: &PatchItem) -> bool {
        use fuzzy_matcher::skim::SkimMatcherV2;
        use fuzzy_matcher::FuzzyMatcher;

        let matcher = SkimMatcherV2::default();

        let matches_state = match self.status {
            Some(patch::Status::Draft) => matches!(patch.state, patch::State::Draft),
            Some(patch::Status::Open) => matches!(patch.state, patch::State::Open { .. }),
            Some(patch::Status::Merged) => matches!(patch.state, patch::State::Merged { .. }),
            Some(patch::Status::Archived) => matches!(patch.state, patch::State::Archived),
            None => true,
        };

        let matches_authored = if self.authored {
            patch.author.you
        } else {
            true
        };

        let matches_authors = (!self.authors.is_empty())
            .then(|| {
                self.authors
                    .iter()
                    .any(|other| patch.author.nid == Some(**other))
            })
            .unwrap_or(true);

        let matches_search = match &self.search {
            Some(search) => match matcher.fuzzy_match(&patch.title, search) {
                Some(score) => score == 0 || score > 60,
                _ => false,
            },
            None => true,
        };

        matches_state && matches_authored && matches_authors && matches_search
    }
}

impl FromStr for PatchItemFilter {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut status = None;
        let mut search = String::new();
        let mut authored = false;
        let mut authors = vec![];

        let mut authors_parser = |input| -> IResult<&str, Vec<&str>> {
            preceded(
                tag("authors:"),
                delimited(
                    tag("["),
                    separated_list0(tag(","), take(56_usize)),
                    tag("]"),
                ),
            )(input)
        };

        let parts = value.split(' ');
        for part in parts {
            match part {
                "is:open" => status = Some(patch::Status::Open),
                "is:merged" => status = Some(patch::Status::Merged),
                "is:archived" => status = Some(patch::Status::Archived),
                "is:draft" => status = Some(patch::Status::Draft),
                "is:authored" => authored = true,
                other => match authors_parser.parse(other) {
                    Ok((_, dids)) => {
                        for did in dids {
                            authors.push(Did::from_str(did)?);
                        }
                    }
                    _ => search.push_str(other),
                },
            }
        }

        let search = if search.is_empty() {
            None
        } else {
            Some(search)
        };

        Ok(Self {
            status,
            authored,
            authors,
            search,
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
}

impl ToTree<String> for CommentItem {
    fn rows(&self) -> Vec<TreeItem<'_, String>> {
        let mut children = vec![];
        for comment in &self.replies {
            children.extend(comment.rows());
        }

        let author = match &self.author.alias {
            Some(alias) => {
                if self.author.you {
                    span::alias(&format!("{} (you)", alias))
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

pub struct TermLine(terminal::Line);

impl<'a> Into<Line<'a>> for TermLine {
    fn into(self) -> Line<'a> {
        Line::raw(self.0.to_string())
    }
}

#[derive(Clone, Debug)]
pub struct ReviewItem<'a> {
    pub inner: IndexedReviewItem,
    pub highlighted: Blobs<Vec<Line<'a>>>,
}

impl<'a> From<(&Repository, &IndexedReviewItem)> for ReviewItem<'a> {
    fn from(value: (&Repository, &IndexedReviewItem)) -> Self {
        let (repo, item) = value;
        let hi = Highlighter::default();

        let blobs = item.1.clone().blobs(repo.raw());
        let highlighted = blobs.highlight(hi);
        Self {
            inner: item.clone(),
            highlighted,
        }
    }
}

impl<'a> ToRow<3> for ReviewItem<'a> {
    fn to_row(&self) -> [Cell; 3] {
        use crate::cob::ReviewItem as Item;
        match &self.inner {
            (
                idx,
                Item::FileAdded {
                    path: _,
                    header: _,
                    new: _,
                    hunk: _,
                    stats: _,
                },
            ) => [
                span::secondary("?").into(),
                span::default(&format!("Hunk {}", idx)).into(),
                span::default("").into(),
            ],
            (idx, Item::FileCopied { copied: _ }) => [
                span::secondary("?").into(),
                span::default(&format!("Hunk {}", idx)).into(),
                span::default("").into(),
            ],
            (
                idx,
                Item::FileDeleted {
                    path: _,
                    header: _,
                    old: _,
                    hunk: _,
                    stats: _,
                },
            ) => [
                span::secondary("?").into(),
                span::default(&format!("Hunk {}", idx)).into(),
                span::default("").into(),
            ],
            (
                idx,
                Item::FileEofChanged {
                    path: _,
                    header: _,
                    old: _,
                    new: _,
                    eof: _,
                },
            ) => [
                span::secondary("?").into(),
                span::default(&format!("Hunk {}", idx)).into(),
                span::default("").into(),
            ],
            (
                idx,
                Item::FileModeChanged {
                    path: _,
                    header: _,
                    old: _,
                    new: _,
                },
            ) => [
                span::secondary("?").into(),
                span::default(&format!("Hunk {}", idx)).into(),
                span::default("").into(),
            ],
            (
                idx,
                Item::FileModified {
                    path: _,
                    header: _,
                    old: _,
                    new: _,
                    hunk: _,
                    stats: _,
                },
            ) => [
                span::secondary("?").into(),
                span::default(&format!("Hunk {}", idx)).into(),
                span::default("").into(),
            ],
            (idx, Item::FileMoved { moved: _ }) => [
                span::secondary("?").into(),
                span::default(&format!("Hunk {}", idx)).into(),
                span::default("").into(),
            ],
        }
    }
}

impl<'a> ReviewItem<'a> {
    pub fn pretty_path(path: &Path, crossed_out: bool) -> Line<'a> {
        let file = path.file_name().unwrap_or_default();
        let path = if path.iter().count() > 1 {
            path.into_iter()
                .take(path.into_iter().count() - 2)
                .map(|component| component.to_string_lossy().to_string())
                .collect::<Vec<_>>()
        } else {
            vec![]
        };

        let line = Line::from(
            [
                if crossed_out {
                    span::default(&file.to_string_lossy().to_string()).crossed_out()
                } else {
                    span::default(&file.to_string_lossy().to_string())
                },
                span::default(" "),
                span::default(&format!("{}", path.join(&String::from("/"))))
                    .dark_gray()
                    .dim(),
            ]
            .to_vec(),
        );
        line.into()
    }
}

/// Blobs passed down to the hunk renderer.
#[derive(Clone, Debug)]
pub struct Blobs<T> {
    pub old: Option<T>,
    pub new: Option<T>,
}

impl<T> Blobs<T> {
    pub fn new(old: Option<T>, new: Option<T>) -> Self {
        Self { old, new }
    }
}

impl<'a> Blobs<(PathBuf, Blob)> {
    pub fn highlight(self, mut hi: Highlighter) -> Blobs<Vec<Line<'a>>> {
        let mut blobs = Blobs::default();
        if let Some((path, Blob::Plain(content))) = &self.old {
            blobs.old = hi
                .highlight(path, content)
                .and_then(|hi| {
                    Ok(hi
                        .into_iter()
                        .map(|line| Line::raw(line.to_string()))
                        .collect::<Vec<_>>())
                })
                .ok();
        }
        if let Some((path, Blob::Plain(content))) = &self.new {
            blobs.new = hi
                .highlight(path, content)
                .and_then(|hi| {
                    Ok(hi
                        .into_iter()
                        .map(|line| Line::raw(line.to_string()))
                        .collect::<Vec<_>>())
                })
                .ok();
        }
        blobs
    }

    pub fn from_paths<R: Repo>(
        old: Option<(&Path, Oid)>,
        new: Option<(&Path, Oid)>,
        repo: &R,
    ) -> Blobs<(PathBuf, Blob)> {
        Blobs::new(
            old.and_then(|(path, oid)| {
                repo.blob(oid)
                    .ok()
                    .or_else(|| repo.file(path))
                    .map(|blob| (path.to_path_buf(), blob))
            }),
            new.and_then(|(path, oid)| {
                repo.blob(oid)
                    .ok()
                    .or_else(|| repo.file(path))
                    .map(|blob| (path.to_path_buf(), blob))
            }),
        )
    }
}

impl<T> Default for Blobs<T> {
    fn default() -> Self {
        Self {
            old: None,
            new: None,
        }
    }
}

pub struct HighlightedLine<'a>(Line<'a>);

impl<'a> From<Line<'a>> for HighlightedLine<'a> {
    fn from(highlighted: Line<'a>) -> Self {
        let converted = highlighted.to_string().into_text().unwrap().lines;

        Self {
            0: converted.first().cloned().unwrap_or_default(),
        }
    }
}

impl<'a> Into<Line<'a>> for HighlightedLine<'a> {
    fn into(self) -> Line<'a> {
        self.0
    }
}

/// Types that can be rendered as texts.
pub trait ToText<'a> {
    /// The output of the render process.
    type Output: Into<Text<'a>>;
    /// Context that can be passed down from parent objects during rendering.
    type Context;

    /// Render to pretty diff output.
    fn to_text<R: Repo>(
        &'a self,
        hi: &mut Highlighter,
        context: &Self::Context,
        repo: &R,
    ) -> Self::Output;
}

impl<'a> ToText<'a> for HunkHeader {
    type Output = Line<'a>;
    type Context = ();

    fn to_text<R: Repo>(
        &self,
        _hi: &mut Highlighter,
        _context: &Self::Context,
        _repo: &R,
    ) -> Self::Output {
        Line::from(
            [
                span::default(&format!(
                    "@@ -{},{} +{},{} @@",
                    self.old_line_no, self.old_size, self.new_line_no, self.new_size,
                ))
                .gray(),
                span::default(" "),
                span::default(&String::from_utf8_lossy(&self.text).to_string()),
            ]
            .to_vec(),
        )
    }
}

impl<'a> ToText<'a> for Modification {
    type Output = Line<'a>;
    type Context = Blobs<Vec<Line<'a>>>;

    fn to_text<R: Repo>(
        &'a self,
        _hi: &mut Highlighter,
        blobs: &Blobs<Vec<Line<'a>>>,
        _repo: &R,
    ) -> Self::Output {
        let line = match self {
            Modification::Deletion(diff::Deletion { line, line_no }) => {
                if let Some(lines) = &blobs.old.as_ref() {
                    lines[*line_no as usize - 1].clone()
                } else {
                    Line::raw(String::from_utf8_lossy(line.as_bytes()))
                }
            }
            Modification::Addition(diff::Addition { line, line_no }) => {
                if let Some(lines) = &blobs.new.as_ref() {
                    lines[*line_no as usize - 1].clone()
                } else {
                    Line::raw(String::from_utf8_lossy(line.as_bytes()))
                }
            }
            Modification::Context {
                line, line_no_new, ..
            } => {
                // Nb. we can check in the old or the new blob, we choose the new.
                if let Some(lines) = &blobs.new.as_ref() {
                    lines[*line_no_new as usize - 1].clone()
                } else {
                    Line::raw(String::from_utf8_lossy(line.as_bytes()))
                }
            }
        };

        HighlightedLine::from(line).into()
    }
}

impl<'a> ToText<'a> for Hunk<Modification> {
    type Output = Vec<Line<'a>>;
    type Context = Blobs<Vec<Line<'a>>>;

    fn to_text<R: Repo>(
        &'a self,
        hi: &mut Highlighter,
        blobs: &Self::Context,
        repo: &R,
    ) -> Self::Output {
        let mut lines: Vec<Line<'a>> = vec![];

        let default_dark = Color::Rgb(20, 20, 20);

        let positive_light = Color::Rgb(10, 60, 20);
        let positive_dark = Color::Rgb(10, 30, 20);

        let negative_light = Color::Rgb(60, 10, 20);
        let negative_dark = Color::Rgb(30, 10, 20);

        if let Ok(header) = HunkHeader::from_bytes(self.header.as_bytes()) {
            lines.push(Line::from(
                [
                    span::default(&format!(
                        "@@ -{},{} +{},{} @@",
                        header.old_line_no, header.old_size, header.new_line_no, header.new_size,
                    ))
                    .gray()
                    .dim(),
                    span::default(" "),
                    span::default(&String::from_utf8_lossy(&header.text).to_string())
                        .gray()
                        .dim(),
                ]
                .to_vec(),
            ))
        }

        for line in &self.lines {
            match line {
                Modification::Addition(a) => {
                    lines.push(Line::from(
                        [
                            [
                                span::positive(&format!("{:<5}", ""))
                                    .bg(positive_light)
                                    .dim(),
                                span::positive(&format!("{:<5}", &a.line_no.to_string()))
                                    .bg(positive_light)
                                    .dim(),
                                span::positive(" + ").bg(positive_dark).dim(),
                            ]
                            .to_vec(),
                            line.to_text(hi, blobs, repo)
                                .spans
                                .into_iter()
                                .map(|span| span.bg(positive_dark))
                                .collect::<Vec<_>>(),
                            [span::positive(&format!("{:<500}", "")).bg(positive_dark)].to_vec(),
                        ]
                        .concat(),
                    ));
                }
                Modification::Deletion(d) => {
                    lines.push(Line::from(
                        [
                            [
                                span::negative(&format!("{:<5}", &d.line_no.to_string()))
                                    .bg(negative_light)
                                    .dim(),
                                span::negative(&format!("{:<5}", ""))
                                    .bg(negative_light)
                                    .dim(),
                                span::negative(" - ").bg(negative_dark).dim(),
                            ]
                            .to_vec(),
                            line.to_text(hi, blobs, repo)
                                .spans
                                .into_iter()
                                .map(|span| span.bg(negative_dark))
                                .collect::<Vec<_>>(),
                            [span::positive(&format!("{:<500}", "")).bg(negative_dark)].to_vec(),
                        ]
                        .concat(),
                    ));
                }
                Modification::Context {
                    line_no_old,
                    line_no_new,
                    ..
                } => {
                    lines.push(Line::from(
                        [
                            [
                                span::default(&format!("{:<5}", &line_no_old.to_string()))
                                    .bg(default_dark)
                                    .gray()
                                    .dim(),
                                span::default(&format!("{:<5}", &line_no_new.to_string()))
                                    .bg(default_dark)
                                    .gray()
                                    .dim(),
                                span::default(&format!("{:<3}", "")),
                            ]
                            .to_vec(),
                            line.to_text(hi, blobs, repo).spans,
                        ]
                        .concat(),
                    ));
                }
            }
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::*;

    #[test]
    fn patch_item_filter_from_str_should_succeed() -> Result<()> {
        let search = r#"is:open is:authored authors:[did:key:z6MkkpTPzcq1ybmjQyQpyre15JUeMvZY6toxoZVpLZ8YarsB,did:key:z6Mku8hpprWTmCv3BqkssCYDfr2feUdyLSUnycVajFo9XVAx] cli"#;
        let actual = PatchItemFilter::from_str(search)?;

        let expected = PatchItemFilter {
            status: Some(patch::Status::Open),
            authored: true,
            authors: vec![
                Did::from_str("did:key:z6MkkpTPzcq1ybmjQyQpyre15JUeMvZY6toxoZVpLZ8YarsB")?,
                Did::from_str("did:key:z6Mku8hpprWTmCv3BqkssCYDfr2feUdyLSUnycVajFo9XVAx")?,
            ],
            search: Some("cli".to_string()),
        };

        assert_eq!(expected, actual);

        Ok(())
    }

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

    #[test]
    fn notification_item_filter_from_str_should_succeed() -> Result<()> {
        let search = r#"is:seen is:patch authors:[did:key:z6MkkpTPzcq1ybmjQyQpyre15JUeMvZY6toxoZVpLZ8YarsB,did:key:z6Mku8hpprWTmCv3BqkssCYDfr2feUdyLSUnycVajFo9XVAx] cli"#;
        let actual = NotificationItemFilter::from_str(search)?;

        let expected = NotificationItemFilter {
            state: Some(NotificationState::Seen),
            type_name: Some(NotificationType::Patch),
            authors: vec![
                Did::from_str("did:key:z6MkkpTPzcq1ybmjQyQpyre15JUeMvZY6toxoZVpLZ8YarsB")?,
                Did::from_str("did:key:z6Mku8hpprWTmCv3BqkssCYDfr2feUdyLSUnycVajFo9XVAx")?,
            ],
            search: Some("cli".to_string()),
        };

        assert_eq!(expected, actual);

        Ok(())
    }
}
