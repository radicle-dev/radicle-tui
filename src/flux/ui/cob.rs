use radicle::crypto::PublicKey;
use radicle::git::Oid;
use radicle::identity::{Did, Identity};
use radicle::node::{Alias, NodeId};
use radicle::{issue, patch, Profile};
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::Cell;

use radicle::cob::{Label, ObjectId, Timestamp, TypedId};
use radicle::issue::{Issue, IssueId, Issues};
use radicle::node::notifications::{Notification, NotificationId, NotificationKind};
use radicle::node::AliasStore;
use radicle::patch::{Patch, PatchId, Patches};
use radicle::storage::git::Repository;
use radicle::storage::{ReadRepository, ReadStorage, RefUpdate};

use super::theme::style;
use super::widget::ToRow;
use super::{format, span};

#[derive(Clone, Debug)]
pub struct AuthorItem {
    pub nid: Option<NodeId>,
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

        Self { nid, alias, you }
    }
}

#[derive(Clone, Debug)]
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

impl ToRow<8> for NotificationItem {
    fn to_row(&self) -> [Cell; 8] {
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

        let id = span::notification_id(format!(" {:-03}", &self.id));
        let seen = if self.seen {
            span::blank()
        } else {
            span::primary(" ● ".into())
        };
        let kind_id = span::primary(kind_id);
        let summary = span::default(summary.to_string());
        let type_name = span::notification_type(type_name);

        let status = match status.as_str() {
            "archived" => span::default(status.to_string()).yellow(),
            "draft" => span::default(status.to_string()).gray().dim(),
            "updated" => span::primary(status.to_string()),
            "open" | "created" => span::positive(status.to_string()),
            "closed" | "merged" => span::ternary(status.to_string()),
            _ => span::default(status.to_string()),
        };
        let author = match &self.author.alias {
            Some(alias) => {
                if self.author.you {
                    span::alias(format!("{} (you)", alias))
                } else {
                    span::alias(alias.to_string())
                }
            }
            None => match self.author.nid {
                Some(nid) => span::alias(format::did(&Did::from(nid))).dim(),
                None => span::alias("".to_string()),
            },
        };

        let timestamp = span::timestamp(format::timestamp(&self.timestamp));

        [
            id.into(),
            seen.into(),
            kind_id.into(),
            summary.into(),
            type_name.into(),
            status.into(),
            author.into(),
            timestamp.into(),
        ]
    }
}

impl ToRow<9> for NotificationItem {
    fn to_row(&self) -> [Cell; 9] {
        let row: [Cell; 8] = self.to_row();
        let name = span::default(self.project.clone()).style(style::gray().dim());

        [
            row[0].clone(),
            row[1].clone(),
            name.into(),
            row[2].clone(),
            row[3].clone(),
            row[4].clone(),
            row[5].clone(),
            row[6].clone(),
            row[7].clone(),
        ]
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
}

impl IssueItem {
    pub fn new(profile: &Profile, issue: (IssueId, Issue)) -> Result<Self, anyhow::Error> {
        let (id, issue) = issue;

        Ok(Self {
            id,
            state: *issue.state(),
            title: issue.title().into(),
            author: AuthorItem {
                nid: Some(*issue.author().id),
                alias: profile.aliases().alias(&issue.author().id),
                you: *issue.author().id == *profile.did(),
            },
            labels: issue.labels().cloned().collect(),
            assignees: issue
                .assignees()
                .map(|did| AuthorItem {
                    nid: Some(**did),
                    alias: profile.aliases().alias(did),
                    you: *did == profile.did(),
                })
                .collect::<Vec<_>>(),
            timestamp: issue.timestamp(),
        })
    }
}

impl ToRow<8> for IssueItem {
    fn to_row(&self) -> [Cell; 8] {
        let (state, state_color) = format_issue_state(&self.state);

        let state = span::default(state).style(Style::default().fg(state_color));
        let id = span::primary(format::cob(&self.id));
        let title = span::default(self.title.clone());

        let author = match &self.author.alias {
            Some(alias) => {
                if self.author.you {
                    span::alias(format!("{} (you)", alias))
                } else {
                    span::alias(alias.to_string())
                }
            }
            None => match self.author.nid {
                Some(nid) => span::alias(format::did(&Did::from(nid))).dim(),
                None => span::alias("".to_string()),
            },
        };
        let did = match self.author.nid {
            Some(nid) => span::alias(format::did(&Did::from(nid))).dim(),
            None => span::alias("".to_string()),
        };
        let labels = span::labels(format_labels(&self.labels));
        let assignees = self
            .assignees
            .iter()
            .map(|author| (author.nid, author.alias.clone(), author.you))
            .collect::<Vec<_>>();
        let assignees = span::alias(format_assignees(&assignees));
        let opened = span::timestamp(format::timestamp(&self.timestamp));

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
        let (_, rev) = patch.latest();
        let repo = radicle_surf::Repository::open(repository.path())?;
        let base = repo.commit(rev.base())?;
        let head = repo.commit(rev.head())?;
        let diff = repo.diff(base.id, head.id)?;

        Ok(Self {
            id,
            state: patch.state().clone(),
            title: patch.title().into(),
            author: AuthorItem {
                nid: Some(*patch.author().id),
                alias: profile.aliases().alias(&patch.author().id),
                you: *patch.author().id == *profile.did(),
            },
            head: rev.head(),
            added: diff.stats().insertions as u16,
            removed: diff.stats().deletions as u16,
            timestamp: rev.timestamp(),
        })
    }
}

impl ToRow<9> for PatchItem {
    fn to_row(&self) -> [Cell; 9] {
        let (state, color) = format_patch_state(&self.state);

        let state = span::default(state).style(Style::default().fg(color));
        let id = span::primary(format::cob(&self.id));
        let title = span::default(self.title.clone());

        let author = match &self.author.alias {
            Some(alias) => {
                if self.author.you {
                    span::alias(format!("{} (you)", alias))
                } else {
                    span::alias(alias.to_string())
                }
            }
            None => match self.author.nid {
                Some(nid) => span::alias(format::did(&Did::from(nid))).dim(),
                None => span::alias("".to_string()),
            },
        };
        let did = match self.author.nid {
            Some(nid) => span::alias(format::did(&Did::from(nid))).dim(),
            None => span::alias("".to_string()),
        };

        let head = span::ternary(format::oid(self.head));
        let added = span::positive(format!("+{}", self.added));
        let removed = span::negative(format!("-{}", self.removed));
        let updated = span::timestamp(format::timestamp(&self.timestamp));

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

pub fn format_issue_state(state: &issue::State) -> (String, Color) {
    match state {
        issue::State::Open => (" ● ".into(), Color::Green),
        issue::State::Closed { reason: _ } => (" ● ".into(), Color::Red),
    }
}

pub fn format_patch_state(state: &patch::State) -> (String, Color) {
    match state {
        patch::State::Open { conflicts: _ } => (" ● ".into(), Color::Green),
        patch::State::Archived => (" ● ".into(), Color::Yellow),
        patch::State::Draft => (" ● ".into(), Color::Gray),
        patch::State::Merged {
            revision: _,
            commit: _,
        } => (" ✔ ".into(), Color::Magenta),
    }
}

pub fn format_labels(labels: &[Label]) -> String {
    let mut output = String::new();
    let mut labels = labels.iter().peekable();

    while let Some(label) = labels.next() {
        output.push_str(&label.to_string());

        if labels.peek().is_some() {
            output.push(',');
        }
    }
    output
}

pub fn format_author(did: &Did, alias: &Option<Alias>, is_you: bool) -> String {
    let author = match alias {
        Some(alias) => format!("{alias}"),
        None => format::did(did),
    };

    if is_you {
        format!("{} (you)", author)
    } else {
        author
    }
}

pub fn format_assignees(assignees: &[(Option<PublicKey>, Option<Alias>, bool)]) -> String {
    let mut output = String::new();
    let mut assignees = assignees.iter().peekable();

    while let Some((assignee, alias, is_you)) = assignees.next() {
        if let Some(assignee) = assignee {
            output.push_str(&format_author(&Did::from(assignee), alias, *is_you));
        }

        if assignees.peek().is_some() {
            output.push(',');
        }
    }
    output
}
