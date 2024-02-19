use anyhow::anyhow;

use ratatui::widgets::Cell;

use radicle::cob::{self, ObjectId, Timestamp};
use radicle::issue::Issues;
use radicle::node::notifications::{Notification, NotificationId, NotificationKind};
use radicle::patch::Patches;
use radicle::storage::git::Repository;
use radicle::storage::{ReadRepository, RefUpdate};

use super::widget::ToRow;
use super::{format, span};

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
                let status = match notification.update {
                    RefUpdate::Updated { .. } => "updated",
                    RefUpdate::Created { .. } => "created",
                    RefUpdate::Deleted { .. } => "deleted",
                    RefUpdate::Skipped { .. } => "skipped",
                };

                Ok(NotificationKindItem::Branch {
                    name: name.to_string(),
                    summary: message,
                    status: status.to_string(),
                    id: head.map(ObjectId::from),
                })
            }
            NotificationKind::Cob { type_name, id } => {
                let (category, summary) = if *type_name == *cob::issue::TYPENAME {
                    let issue = issues.get(id)?.ok_or(anyhow!("missing"))?;
                    (String::from("issue"), issue.title().to_owned())
                } else if *type_name == *cob::patch::TYPENAME {
                    let patch = patches.get(id)?.ok_or(anyhow!("missing"))?;
                    (String::from("patch"), patch.title().to_owned())
                } else {
                    (type_name.to_string(), "".to_owned())
                };
                let status = match notification.update {
                    RefUpdate::Updated { .. } => "updated",
                    RefUpdate::Created { .. } => "opened",
                    RefUpdate::Deleted { .. } => "deleted",
                    RefUpdate::Skipped { .. } => "skipped",
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
    /// Time the update has happened.
    timestamp: Timestamp,
}

impl TryFrom<(&Repository, &Notification)> for NotificationItem {
    type Error = anyhow::Error;

    fn try_from(value: (&Repository, &Notification)) -> Result<Self, Self::Error> {
        let (repo, notification) = value;
        let kind = NotificationKindItem::try_from((repo, notification))?;

        Ok(NotificationItem {
            id: notification.id,
            seen: notification.status.is_read(),
            kind,
            timestamp: notification.timestamp.into(),
        })
    }
}

impl ToRow<7> for NotificationItem {
    fn to_row(&self) -> [Cell; 7] {
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

        let id = span::default(format!(" {}", &self.id));
        let seen = if self.seen {
            span::blank()
        } else {
            span::positive(" ● ".into())
        };
        let type_name = span::secondary(type_name);
        let summary = span::default(summary.to_string());
        let kind_id = span::primary(kind_id);
        let status = span::default(status.to_string());
        let timestamp = span::timestamp(format::timestamp(&self.timestamp));

        [
            id.into(),
            seen.into(),
            type_name.into(),
            summary.into(),
            kind_id.into(),
            status.into(),
            timestamp.into(),
        ]
    }
}
