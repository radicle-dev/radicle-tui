use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;
use std::str::FromStr;

use nom::bytes::complete::{tag, take};
use nom::multi::separated_list0;
use nom::sequence::{delimited, preceded};
use nom::{IResult, Parser};

use ansi_to_tui::IntoText;

use radicle::cob::thread::{Comment, CommentId};
use radicle::cob::{CodeLocation, CodeRange, EntryId, Label, ObjectId, Timestamp, TypedId};
use radicle::git::Oid;
use radicle::identity::{Did, Identity};
use radicle::issue;
use radicle::issue::{CloseReason, Issue, IssueId, Issues};
use radicle::node::notifications::{Notification, NotificationId, NotificationKind};
use radicle::node::{Alias, AliasStore, NodeId};
use radicle::patch::{self, Review};
use radicle::patch::{Patch, PatchId, Patches};
use radicle::storage::git::Repository;
use radicle::storage::{ReadRepository, ReadStorage, RefUpdate, WriteRepository};
use radicle::Profile;

use radicle_surf::diff::{self, Hunk, Modification};

use radicle_cli::git::unified_diff::{Decode, HunkHeader};
use radicle_cli::terminal;
use radicle_cli::terminal::highlight::Highlighter;

use ratatui::prelude::*;
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::Cell;

use tui_tree_widget::TreeItem;

use radicle_tui as tui;

use tui::ui::theme::style;
use tui::ui::utils::LineMerger;
use tui::ui::{span, Column};
use tui::ui::{ToRow, ToTree};

use crate::git::{Blobs, DiffStats, HunkDiff, HunkStats, StatefulHunkDiff};
use crate::ui;

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

impl<'a> From<TermLine> for Line<'a> {
    fn from(val: TermLine) -> Self {
        Line::raw(val.0.to_string())
    }
}

/// All comments per hunk, indexed by their starting line.
#[derive(Clone, Debug)]
pub struct HunkComments {
    /// All comments. Can be unsorted.
    comments: HashMap<usize, Vec<(EntryId, Comment<CodeLocation>)>>,
}

impl HunkComments {
    pub fn all(&self) -> &HashMap<usize, Vec<(EntryId, Comment<CodeLocation>)>> {
        &self.comments
    }

    pub fn is_empty(&self) -> bool {
        self.comments.is_empty()
    }

    pub fn len(&self) -> usize {
        self.comments.values().fold(0_usize, |mut count, comments| {
            count += comments.len();
            count
        })
    }
}

impl From<Vec<(EntryId, Comment<CodeLocation>)>> for HunkComments {
    fn from(comments: Vec<(EntryId, Comment<CodeLocation>)>) -> Self {
        let mut line_comments: HashMap<usize, Vec<(EntryId, Comment<CodeLocation>)>> =
            HashMap::new();

        for comment in comments {
            // TODO(erikli): Check why we need range end instead of range start.
            let line = match comment.1.location().as_ref().unwrap().new.as_ref().unwrap() {
                CodeRange::Lines { range } => range.end,
                _ => 0,
            };

            if let Some(comments) = line_comments.get_mut(&line) {
                comments.push(comment.clone());
            } else {
                line_comments.insert(line, vec![comment.clone()]);
            }
        }

        Self {
            comments: line_comments,
        }
    }
}

/// A [`HunkItem`] that can be rendered. Hunk items are indexed sequentially and
/// provide access to the underlying hunk type.
#[derive(Clone)]
pub struct HunkItem<'a> {
    /// The underlying hunk type and its current state (accepted / rejected).
    pub inner: StatefulHunkDiff,
    /// Raw or highlighted hunk lines. Highlighting is expensive and needs to be asynchronously.
    /// Therefor, a hunks' lines need to stored separately.
    pub lines: Blobs<Vec<Line<'a>>>,
    /// A hunks' comments, indexed by line.
    pub comments: HunkComments,
}

impl<'a> From<(&Repository, &Review, StatefulHunkDiff)> for HunkItem<'a> {
    fn from(value: (&Repository, &Review, StatefulHunkDiff)) -> Self {
        let (repo, review, item) = value;
        let hi = Highlighter::default();
        let hunk = item.hunk();

        let path = match &hunk {
            HunkDiff::Added { path, .. } => path,
            HunkDiff::Modified { path, .. } => path,
            HunkDiff::Deleted { path, .. } => path,
            HunkDiff::Copied { copied } => &copied.new_path,
            HunkDiff::Moved { moved } => &moved.new_path,
            HunkDiff::ModeChanged { path, .. } => path,
            HunkDiff::EofChanged { path, .. } => path,
        };

        // TODO(erikli): Start with raw, non-highlighted lines and
        // move highlighting to separate task / thread, e.g. here:
        // `let lines = blobs.raw()`
        let blobs = hunk.clone().blobs(repo.raw());
        let lines = blobs.highlight(hi);
        let comments = review
            .comments()
            .filter(|(_, comment)| comment.location().is_some())
            .filter(|(_, comment)| comment.location().unwrap().path == *path)
            .map(|(id, comment)| (*id, comment.clone()))
            .collect::<Vec<_>>();

        Self {
            inner: item.clone(),
            lines,
            comments: HunkComments::from(comments),
        }
    }
}

impl<'a> ToRow<3> for HunkItem<'a> {
    fn to_row(&self) -> [Cell; 3] {
        let build_stats_spans = |stats: &DiffStats| -> Vec<Span<'_>> {
            let mut cell = vec![];

            if !self.comments.is_empty() {
                cell.push(
                    span::default(&format!(" {} ", self.comments.len()))
                        .dim()
                        .reversed(),
                );
                cell.push(span::default(" "));
            }

            let (added, deleted) = match stats {
                DiffStats::Hunk(stats) => (stats.added(), stats.deleted()),
                DiffStats::File(stats) => (stats.additions, stats.deletions),
            };

            if added > 0 {
                cell.push(span::default(&format!("+{}", added)).light_green().dim());
            }

            if added > 0 && deleted > 0 {
                cell.push(span::default(",").dim());
            }

            if deleted > 0 {
                cell.push(span::default(&format!("-{}", deleted)).light_red().dim());
            }

            cell
        };

        match &self.inner.hunk() {
            HunkDiff::Added {
                path,
                header: _,
                new: _,
                hunk,
                _stats: _,
            } => {
                let stats = hunk.as_ref().map(HunkStats::from).unwrap_or_default();
                let stats_cell = [
                    build_stats_spans(&DiffStats::Hunk(stats)),
                    [span::default(" A ").bold().light_green().dim()].to_vec(),
                ]
                .concat();

                [
                    Line::from(ui::span::hunk_state(self.inner.state()))
                        .right_aligned()
                        .into(),
                    Line::from(ui::span::pretty_path(path, false, false)).into(),
                    Line::from(stats_cell).right_aligned().into(),
                ]
            }
            HunkDiff::Modified {
                path,
                header: _,
                old: _,
                new: _,
                hunk,
                _stats: _,
            } => {
                let stats = hunk.as_ref().map(HunkStats::from).unwrap_or_default();
                let stats_cell = [
                    build_stats_spans(&DiffStats::Hunk(stats)),
                    [span::default(" M ").bold().light_yellow().dim()].to_vec(),
                ]
                .concat();

                [
                    Line::from(ui::span::hunk_state(self.inner.state()))
                        .right_aligned()
                        .into(),
                    Line::from(ui::span::pretty_path(path, false, false)).into(),
                    Line::from(stats_cell).right_aligned().into(),
                ]
            }
            HunkDiff::Deleted {
                path,
                header: _,
                old: _,
                hunk,
                _stats: _,
            } => {
                let stats = hunk.as_ref().map(HunkStats::from).unwrap_or_default();
                let stats_cell = [
                    build_stats_spans(&DiffStats::Hunk(stats)),
                    [span::default(" D ").bold().light_red().dim()].to_vec(),
                ]
                .concat();

                [
                    Line::from(ui::span::hunk_state(self.inner.state()))
                        .right_aligned()
                        .into(),
                    Line::from(ui::span::pretty_path(path, false, false)).into(),
                    Line::from(stats_cell).right_aligned().into(),
                ]
            }
            HunkDiff::Copied { copied } => {
                let stats = copied.diff.stats().copied().unwrap_or_default();
                let stats_cell = [
                    build_stats_spans(&DiffStats::File(stats)),
                    [span::default(" CP ").bold().light_blue().dim()].to_vec(),
                ]
                .concat();

                [
                    Line::from(ui::span::hunk_state(self.inner.state()))
                        .right_aligned()
                        .into(),
                    Line::from(ui::span::pretty_path(&copied.new_path, false, false)).into(),
                    Line::from(stats_cell).right_aligned().into(),
                ]
            }
            HunkDiff::Moved { moved } => {
                let stats = moved.diff.stats().copied().unwrap_or_default();
                let stats_cell = [
                    build_stats_spans(&DiffStats::File(stats)),
                    [span::default(" MV ").bold().light_blue().dim()].to_vec(),
                ]
                .concat();

                [
                    Line::from(ui::span::hunk_state(self.inner.state()))
                        .right_aligned()
                        .into(),
                    Line::from(ui::span::pretty_path(&moved.new_path, false, false)).into(),
                    Line::from(stats_cell).right_aligned().into(),
                ]
            }
            HunkDiff::EofChanged {
                path,
                header: _,
                old: _,
                new: _,
                _eof: _,
            } => [
                Line::from(ui::span::hunk_state(self.inner.state()))
                    .right_aligned()
                    .into(),
                Line::from(ui::span::pretty_path(path, false, false)).into(),
                Line::from(span::default("EOF ").light_blue())
                    .right_aligned()
                    .into(),
            ],
            HunkDiff::ModeChanged {
                path,
                header: _,
                old: _,
                new: _,
            } => [
                Line::from(ui::span::hunk_state(self.inner.state()))
                    .right_aligned()
                    .into(),
                Line::from(ui::span::pretty_path(path, false, false)).into(),
                Line::from(span::default("FM ").light_blue())
                    .right_aligned()
                    .into(),
            ],
        }
    }
}

impl<'a> HunkItem<'a> {
    pub fn header(&self) -> Vec<Column<'a>> {
        let comment_tag = if !self.comments.is_empty() {
            let count = self.comments.len();
            if count == 1 {
                span::default(" 1 comment ").dim().reversed()
            } else {
                span::default(&format!(" {} comments ", count))
                    .dim()
                    .reversed()
            }
        } else {
            span::blank()
        };

        match &self.inner.hunk() {
            HunkDiff::Added {
                path,
                header: _,
                new: _,
                hunk: _,
                _stats: _,
            } => {
                let path = Line::from(ui::span::pretty_path(path, false, true));
                let header = [
                    Column::new("", Constraint::Length(0)),
                    Column::new(path.clone(), Constraint::Length(path.width() as u16)),
                    Column::new(
                        Line::from(
                            [
                                comment_tag,
                                span::default(" "),
                                span::default(" added ").light_green().dim().reversed(),
                            ]
                            .to_vec(),
                        )
                        .right_aligned(),
                        Constraint::Fill(1),
                    ),
                ];

                header.to_vec()
            }

            HunkDiff::Modified {
                path,
                header: _,
                old: _,
                new: _,
                hunk: _,
                _stats: _,
            } => {
                let path = Line::from(ui::span::pretty_path(path, false, true));
                let header = [
                    Column::new("", Constraint::Length(0)),
                    Column::new(path.clone(), Constraint::Length(path.width() as u16)),
                    Column::new(
                        Line::from(
                            [
                                comment_tag,
                                span::default(" "),
                                span::default(" modified ").light_yellow().dim().reversed(),
                            ]
                            .to_vec(),
                        )
                        .right_aligned(),
                        Constraint::Fill(1),
                    ),
                ];

                header.to_vec()
            }

            HunkDiff::Deleted {
                path,
                header: _,
                old: _,
                hunk: _,
                _stats: _,
            } => {
                let path = Line::from(ui::span::pretty_path(path, true, true));
                let header = [
                    Column::new("", Constraint::Length(0)),
                    Column::new(path.clone(), Constraint::Length(path.width() as u16)),
                    Column::new(
                        Line::from(
                            [
                                comment_tag,
                                span::default(" "),
                                span::default(" deleted ").light_red().dim().reversed(),
                            ]
                            .to_vec(),
                        )
                        .right_aligned(),
                        Constraint::Fill(1),
                    ),
                ];

                header.to_vec()
            }
            HunkDiff::Copied { copied } => {
                let path = Line::from(
                    [
                        ui::span::pretty_path(&copied.old_path, false, true),
                        [span::default(" -> ")].to_vec(),
                        ui::span::pretty_path(&copied.new_path, false, true),
                    ]
                    .concat()
                    .to_vec(),
                );
                let header = [
                    Column::new("", Constraint::Length(0)),
                    Column::new(path.clone(), Constraint::Length(path.width() as u16)),
                    Column::new(
                        span::default(" copied ")
                            .light_blue()
                            .dim()
                            .reversed()
                            .into_right_aligned_line(),
                        Constraint::Fill(1),
                    ),
                ];

                header.to_vec()
            }
            HunkDiff::Moved { moved } => {
                let path = Line::from(
                    [
                        ui::span::pretty_path(&moved.old_path, false, true),
                        [span::default(" -> ")].to_vec(),
                        ui::span::pretty_path(&moved.new_path, false, true),
                    ]
                    .concat()
                    .to_vec(),
                );
                let header = [
                    Column::new("", Constraint::Length(0)),
                    Column::new(path.clone(), Constraint::Length(path.width() as u16)),
                    Column::new(
                        span::default(" moved ")
                            .light_blue()
                            .dim()
                            .reversed()
                            .into_right_aligned_line(),
                        Constraint::Fill(1),
                    ),
                ];

                header.to_vec()
            }

            HunkDiff::EofChanged {
                path,
                header: _,
                old: _,
                new: _,
                _eof: _,
            } => {
                let path = Line::from(ui::span::pretty_path(path, false, true));
                let header = [
                    Column::new("", Constraint::Length(0)),
                    Column::new(path.clone(), Constraint::Length(path.width() as u16)),
                    Column::new(
                        span::default(" eof ")
                            .dim()
                            .reversed()
                            .into_right_aligned_line(),
                        Constraint::Fill(1),
                    ),
                ];

                header.to_vec()
            }
            HunkDiff::ModeChanged {
                path,
                header: _,
                old: _,
                new: _,
            } => {
                let path = Line::from(ui::span::pretty_path(path, false, true));
                let header = [
                    Column::new("", Constraint::Length(0)),
                    Column::new(path.clone(), Constraint::Length(path.width() as u16)),
                    Column::new(
                        span::default(" mode ")
                            .dim()
                            .reversed()
                            .into_right_aligned_line(),
                        Constraint::Length(6),
                    ),
                ];

                header.to_vec()
            }
        }
    }

    pub fn hunk_text(&'a self) -> Option<Text<'a>> {
        match &self.inner.hunk() {
            HunkDiff::Added { hunk, .. }
            | HunkDiff::Modified { hunk, .. }
            | HunkDiff::Deleted { hunk, .. } => {
                let mut lines = hunk
                    .as_ref()
                    .map(|hunk| Text::from(hunk.to_text(&self.lines)));
                let start = hunk
                    .as_ref()
                    .map(|hunk| hunk.new.start as usize)
                    .unwrap_or_default();

                lines = lines.map(|lines| {
                    let mut mixins = HashMap::new();

                    let divider = span::default(&"─".to_string().repeat(500)).gray().dim();

                    for (line, comments) in self.comments.all() {
                        mixins.insert(
                            *line,
                            comments
                                .iter()
                                .enumerate()
                                .map(|(idx, comment)| {
                                    // let body = span::default(comment.1.body()).gray();
                                    let timestamp =
                                        span::timestamp(&format::timestamp(&comment.1.timestamp()));
                                    let author =
                                        span::alias(&format::did(&Did::from(comment.1.author())));

                                    let mut rendered = vec![];

                                    // Only add top divider for the first comment
                                    if idx == 0 {
                                        rendered.push(Line::from([divider.clone()].to_vec()));
                                    }

                                    // Add comment body
                                    rendered.extend(
                                        comment
                                            .1
                                            .body()
                                            .lines()
                                            .map(|line| {
                                                Line::from([span::default(line).gray()].to_vec())
                                            })
                                            .collect::<Vec<_>>(),
                                    );

                                    // Add metadata
                                    rendered.push(
                                        Line::from(
                                            [timestamp, span::default(" by ").dim(), author]
                                                .to_vec(),
                                        )
                                        .right_aligned(),
                                    );

                                    // Add bottom divider
                                    rendered.push(Line::from([divider.clone()].to_vec()));

                                    rendered
                                })
                                .collect(),
                        );
                    }
                    let merged = LineMerger::merge(lines.lines.clone(), mixins, start);

                    Text::from(merged)
                });

                lines
            }
            _ => None,
        }
    }
}

impl<'a> Debug for HunkItem<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HunkItem")
            .field("inner", &self.inner)
            .field("comments", &self.comments)
            .finish()
    }
}

pub struct HighlightedLine<'a>(Line<'a>);

impl<'a> From<Line<'a>> for HighlightedLine<'a> {
    fn from(highlighted: Line<'a>) -> Self {
        let converted = highlighted.to_string().into_text().unwrap().lines;

        Self(converted.first().cloned().unwrap_or_default())
    }
}

impl<'a> From<HighlightedLine<'a>> for Line<'a> {
    fn from(val: HighlightedLine<'a>) -> Self {
        val.0
    }
}

/// Types that can be rendered as texts.
pub trait ToText<'a> {
    /// The output of the render process.
    type Output: Into<Text<'a>>;
    /// Context that can be passed down from parent objects during rendering.
    type Context;

    /// Render to pretty diff output.
    fn to_text(&'a self, context: &Self::Context) -> Self::Output;
}

impl<'a> ToText<'a> for HunkHeader {
    type Output = Line<'a>;
    type Context = ();

    fn to_text(&self, _context: &Self::Context) -> Self::Output {
        Line::from(
            [
                span::default(&format!(
                    "@@ -{},{} +{},{} @@",
                    self.old_line_no, self.old_size, self.new_line_no, self.new_size,
                ))
                .gray(),
                span::default(" "),
                span::default(String::from_utf8_lossy(&self.text).as_ref()),
            ]
            .to_vec(),
        )
    }
}

impl<'a> ToText<'a> for Modification {
    type Output = Line<'a>;
    type Context = Blobs<Vec<Line<'a>>>;

    fn to_text(&'a self, blobs: &Blobs<Vec<Line<'a>>>) -> Self::Output {
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

    fn to_text(&'a self, blobs: &Self::Context) -> Self::Output {
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
                    span::default(String::from_utf8_lossy(&header.text).as_ref())
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
                            line.to_text(blobs)
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
                            line.to_text(blobs)
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
                            line.to_text(blobs).spans,
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
