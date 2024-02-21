use radicle::identity::Did;
use radicle::node::{Alias, NodeId};
use radicle::Profile;
use ratatui::style::Stylize;
use ratatui::widgets::Cell;

use radicle::cob::{self, ObjectId, Timestamp};
use radicle::issue::Issues;
use radicle::node::notifications::{Notification, NotificationId, NotificationKind};
use radicle::node::AliasStore;
use radicle::patch::Patches;
use radicle::storage::git::Repository;
use radicle::storage::{ReadRepository, RefUpdate};

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
}

impl TryFrom<(&Repository, &Notification)> for NotificationKindItem {
    type Error = anyhow::Error;

    fn try_from(value: (&Repository, &Notification)) -> Result<Self, Self::Error> {
        let (repo, notification) = value;
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

                Ok(NotificationKindItem::Branch {
                    name: name.to_string(),
                    summary: message,
                    status: status.to_string(),
                    id: head.map(ObjectId::from),
                })
            }
            NotificationKind::Cob { type_name, id } => {
                let (category, summary, status) = if *type_name == *cob::issue::TYPENAME {
                    let Some(issue) = issues.get(id)? else {
                        // Issue could have been deleted after notification was created.
                        anyhow::bail!("Issue deleted after notification was created");
                    };
                    (
                        String::from("issue"),
                        issue.title().to_owned(),
                        issue.state().to_string(),
                    )
                } else if *type_name == *cob::patch::TYPENAME {
                    let Some(patch) = patches.get(id)? else {
                        // Patch could have been deleted after notification was created.
                        anyhow::bail!("patch deleted after notification was created");
                    };
                    (
                        String::from("patch"),
                        patch.title().to_owned(),
                        patch.state().to_string(),
                    )
                } else {
                    (type_name.to_string(), "".to_owned(), String::new())
                };

                Ok(NotificationKindItem::Cob {
                    type_name: category.to_string(),
                    summary: summary.to_string(),
                    status: status.to_string(),
                    id: Some(*id),
                })
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct NotificationItem {
    /// Unique notification ID.
    pub id: NotificationId,
    /// Mark this notification as seen.
    pub seen: bool,
    /// Wrapped notification kind.
    pub kind: NotificationKindItem,
    /// The author
    pub author: AuthorItem,
    /// Time the update has happened.
    pub timestamp: Timestamp,
}

impl TryFrom<(&Profile, &Repository, &Notification)> for NotificationItem {
    type Error = anyhow::Error;

    fn try_from(value: (&Profile, &Repository, &Notification)) -> Result<Self, Self::Error> {
        let (profile, repo, notification) = value;
        let kind = NotificationKindItem::try_from((repo, notification))?;

        Ok(NotificationItem {
            id: notification.id,
            seen: notification.status.is_read(),
            kind,
            author: AuthorItem::new(notification.remote, profile),
            timestamp: notification.timestamp.into(),
        })
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
            } => ("branch".to_string(), summary, status, name.to_string()),
            NotificationKindItem::Cob {
                type_name,
                summary,
                status,
                id,
            } => {
                let id = id.map(|id| format::cob(&id)).unwrap_or_default();
                (type_name.to_string(), summary, status, id.to_string())
            }
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
