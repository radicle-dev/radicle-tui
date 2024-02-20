pub mod format;

use anyhow::anyhow;

use radicle_surf;

use tuirealm::props::{Color, Style};
use tuirealm::tui::text::Line;
use tuirealm::tui::widgets::Cell;

use radicle::cob::issue::{self, Issue, IssueId};
use radicle::cob::patch::{self, Patch, PatchId};
use radicle::cob::{Label, ObjectId, Timestamp};
use radicle::issue::Issues;
use radicle::node::notifications::{Notification, NotificationId, NotificationKind};
use radicle::node::{Alias, AliasStore};
use radicle::patch::Patches;
use radicle::prelude::Did;
use radicle::storage::git::Repository;
use radicle::storage::{Oid, ReadRepository, RefUpdate};
use radicle::{cob, Profile};

use super::super::ui::theme::Theme;
use super::super::ui::widget::list::{ListItem, TableItem};

use super::widget::label;

/// An author item that can be used in tables, list or trees.
///
/// Breaks up dependencies to [`Profile`] and [`Repository`] that
/// would be needed if [`AuthorItem`] would be used directly.
#[derive(Clone)]
pub struct AuthorItem {
    /// The author's DID.
    did: Did,
    /// The author's alias
    alias: Option<Alias>,
    /// True if the author is the current user.
    is_you: bool,
}

impl AuthorItem {
    pub fn did(&self) -> Did {
        self.did
    }

    pub fn is_you(&self) -> bool {
        self.is_you
    }

    pub fn alias(&self) -> Option<Alias> {
        self.alias.clone()
    }
}

/// A patch item that can be used in tables, list or trees.
///
/// Breaks up dependencies to [`Profile`] and [`Repository`] that
/// would be needed if [`Patch`] would be used directly.
#[derive(Clone)]
pub struct PatchItem {
    /// Patch OID.
    id: PatchId,
    /// Patch state.
    state: patch::State,
    /// Patch title.
    title: String,
    /// Author of the latest revision.
    author: AuthorItem,
    /// Head of the latest revision.
    head: Oid,
    /// Lines added by the latest revision.
    added: u16,
    /// Lines removed by the latest revision.
    removed: u16,
    /// Time when patch was opened.
    timestamp: Timestamp,
}

impl PatchItem {
    pub fn id(&self) -> &PatchId {
        &self.id
    }

    pub fn state(&self) -> &patch::State {
        &self.state
    }

    pub fn title(&self) -> &String {
        &self.title
    }

    pub fn author(&self) -> &AuthorItem {
        &self.author
    }

    pub fn head(&self) -> &Oid {
        &self.head
    }

    pub fn added(&self) -> u16 {
        self.added
    }

    pub fn removed(&self) -> u16 {
        self.removed
    }

    pub fn timestamp(&self) -> &Timestamp {
        &self.timestamp
    }
}

impl PartialEq for PatchItem {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl TryFrom<(&Profile, &Repository, PatchId, Patch)> for PatchItem {
    type Error = anyhow::Error;

    fn try_from(value: (&Profile, &Repository, PatchId, Patch)) -> Result<Self, Self::Error> {
        let (profile, repo, id, patch) = value;
        let (_, rev) = patch.latest();
        let repo = radicle_surf::Repository::open(repo.path())?;
        let base = repo.commit(rev.base())?;
        let head = repo.commit(rev.head())?;
        let diff = repo.diff(base.id, head.id)?;
        let author = patch.author().id;

        Ok(PatchItem {
            id,
            state: patch.state().clone(),
            title: patch.title().into(),
            author: AuthorItem {
                did: author,
                alias: profile.aliases().alias(&author),
                is_you: *patch.author().id == *profile.did(),
            },
            head: rev.head(),
            added: diff.stats().insertions as u16,
            removed: diff.stats().deletions as u16,
            timestamp: rev.timestamp(),
        })
    }
}

impl TableItem<8> for PatchItem {
    fn row(&self, _theme: &Theme, highlight: bool) -> [Cell; 8] {
        let (icon, color) = format_patch_state(&self.state);

        if highlight {
            let state = label::reversed(&icon).into();
            let id = label::reversed(&format::cob(&self.id)).into();
            let title = label::reversed(&self.title.clone()).into();

            let author = label::reversed(&format_author(
                &self.author.did,
                &self.author.alias,
                self.author.is_you,
            ))
            .into();

            let head = label::reversed(&format::oid(self.head)).into();
            let added = label::reversed(&format!("+{}", self.added)).into();
            let removed = label::reversed(&format!("-{}", self.removed)).into();
            let updated = label::reversed(&format::timestamp(&self.timestamp)).into();

            [state, id, title, author, head, added, removed, updated]
        } else {
            let state = label::default(&icon)
                .style(Style::default().fg(color))
                .into();
            let id = label::id(&format::cob(&self.id)).into();
            let title = label::default(&self.title.clone()).into();

            let author = match &self.author.alias {
                Some(_) => label::alias(&format_author(
                    &self.author.did,
                    &self.author.alias,
                    self.author.is_you,
                ))
                .into(),
                None => label::did(&format_author(
                    &self.author.did,
                    &self.author.alias,
                    self.author.is_you,
                ))
                .into(),
            };

            let head = label::oid(&format::oid(self.head)).into();
            let added = label::positive(&format!("+{}", self.added)).into();
            let removed = label::negative(&format!("-{}", self.removed)).into();
            let updated = label::timestamp(&format::timestamp(&self.timestamp)).into();

            [state, id, title, author, head, added, removed, updated]
        }
    }
}

/// An issue item that can be used in tables, list or trees.
///
/// Breaks up dependencies to [`Profile`] and [`Repository`] that
/// would be needed if [`Issue`] would be used directly.
#[derive(Clone)]
pub struct IssueItem {
    /// Issue OID.
    id: IssueId,
    /// Issue state.
    state: issue::State,
    /// Issue title.
    title: String,
    /// Issue author.
    author: AuthorItem,
    /// Issue labels.
    labels: Vec<Label>,
    /// Issue assignees.
    assignees: Vec<AuthorItem>,
    /// Time when issue was opened.
    timestamp: Timestamp,
}

impl IssueItem {
    pub fn id(&self) -> &IssueId {
        &self.id
    }

    pub fn state(&self) -> &issue::State {
        &self.state
    }

    pub fn title(&self) -> &String {
        &self.title
    }

    pub fn author(&self) -> &AuthorItem {
        &self.author
    }

    pub fn labels(&self) -> &Vec<Label> {
        &self.labels
    }

    pub fn assignees(&self) -> &Vec<AuthorItem> {
        &self.assignees
    }

    pub fn timestamp(&self) -> &Timestamp {
        &self.timestamp
    }
}

impl From<(&Profile, &Repository, IssueId, Issue)> for IssueItem {
    fn from(value: (&Profile, &Repository, IssueId, Issue)) -> Self {
        let (profile, _, id, issue) = value;
        let author = issue.author().id;

        IssueItem {
            id,
            state: *issue.state(),
            title: issue.title().into(),
            author: AuthorItem {
                did: issue.author().id,
                alias: profile.aliases().alias(&author),
                is_you: *issue.author().id == *profile.did(),
            },
            labels: issue.labels().cloned().collect(),
            assignees: issue
                .assignees()
                .map(|did| AuthorItem {
                    did: *did,
                    alias: profile.aliases().alias(did),
                    is_you: *did == profile.did(),
                })
                .collect::<Vec<_>>(),
            timestamp: issue.timestamp(),
        }
    }
}

impl TableItem<7> for IssueItem {
    fn row(&self, _theme: &Theme, highlight: bool) -> [Cell; 7] {
        let (icon, color) = format_issue_state(&self.state);

        if highlight {
            let state = label::reversed(&icon).into();
            let id = label::reversed(&format::cob(&self.id)).into();
            let title = label::reversed(&self.title.clone()).into();

            let author = label::reversed(&format_author(
                &self.author.did,
                &self.author.alias,
                self.author.is_you,
            ))
            .into();

            let labels = label::reversed(&format_labels(&self.labels)).into();
            let assignees = self
                .assignees
                .iter()
                .map(|author| (author.did, author.alias.clone(), author.is_you))
                .collect::<Vec<_>>();
            let assignees = label::reversed(&format_assignees(&assignees)).into();
            let opened = label::reversed(&format::timestamp(&self.timestamp)).into();

            [state, id, title, author, labels, assignees, opened]
        } else {
            let state = label::default(&icon)
                .style(Style::default().fg(color))
                .into();
            let id = label::id(&format::cob(&self.id)).into();
            let title = label::default(&self.title.clone()).into();

            let author = match &self.author.alias {
                Some(_) => label::alias(&format_author(
                    &self.author.did,
                    &self.author.alias,
                    self.author.is_you,
                ))
                .into(),
                None => label::did(&format_author(
                    &self.author.did,
                    &self.author.alias,
                    self.author.is_you,
                ))
                .into(),
            };

            let labels = label::labels(&format_labels(&self.labels)).into();
            let assignees = self
                .assignees
                .iter()
                .map(|author| (author.did, author.alias.clone(), author.is_you))
                .collect::<Vec<_>>();
            let assignees = label::did(&format_assignees(&assignees)).into();
            let opened = label::timestamp(&format::timestamp(&self.timestamp)).into();

            [state, id, title, author, labels, assignees, opened]
        }
    }
}

impl ListItem for IssueItem {
    fn row(&self, theme: &Theme) -> tuirealm::tui::widgets::ListItem {
        let (state, state_color) = format_issue_state(&self.state);

        let lines = vec![
            Line::from(vec![
                label::default(&state)
                    .style(Style::default().fg(state_color))
                    .into(),
                label::title(&self.title).into(),
            ]),
            Line::from(vec![
                label::default("   ").into(),
                match &self.author.alias {
                    Some(_) => label::alias(&format_author(
                        &self.author.did,
                        &self.author.alias,
                        self.author.is_you,
                    ))
                    .into(),
                    None => label::did(&format_author(
                        &self.author.did,
                        &self.author.alias,
                        self.author.is_you,
                    ))
                    .into(),
                },
                label::property_divider(&format!(" {} ", theme.icons.property_divider)).into(),
                label::timestamp(&format::timestamp(&self.timestamp)).into(),
            ]),
        ];
        tuirealm::tui::widgets::ListItem::new(lines)
    }
}

impl PartialEq for IssueItem {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

//////////////////////////////////////////////////////
#[derive(Clone)]
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

impl TryFrom<(&Repository, NotificationKind, RefUpdate)> for NotificationKindItem {
    type Error = anyhow::Error;

    fn try_from(value: (&Repository, NotificationKind, RefUpdate)) -> Result<Self, Self::Error> {
        let (repo, kind, update) = value;
        let issues = Issues::open(repo)?;
        let patches = Patches::open(repo)?;

        match kind {
            NotificationKind::Branch { name } => {
                let (head, message) = if let Some(head) = update.new() {
                    let message = repo.commit(head)?.summary().unwrap_or_default().to_owned();
                    (Some(head), message)
                } else {
                    (None, String::new())
                };
                let status = match update {
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
                let (category, summary) = if type_name == *cob::issue::TYPENAME {
                    let issue = issues.get(&id)?.ok_or(anyhow!("missing"))?;
                    (String::from("issue"), issue.title().to_owned())
                } else if type_name == *cob::patch::TYPENAME {
                    let patch = patches.get(&id)?.ok_or(anyhow!("missing"))?;
                    (String::from("patch"), patch.title().to_owned())
                } else {
                    (type_name.to_string(), "".to_owned())
                };
                let status = match update {
                    RefUpdate::Updated { .. } => "updated",
                    RefUpdate::Created { .. } => "opened",
                    RefUpdate::Deleted { .. } => "deleted",
                    RefUpdate::Skipped { .. } => "skipped",
                };

                Ok(NotificationKindItem::Cob {
                    type_name: category.to_string(),
                    summary: summary.to_string(),
                    status: status.to_string(),
                    id: Some(id),
                })
            }
        }
    }
}

#[derive(Clone)]
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

impl NotificationItem {
    pub fn id(&self) -> &NotificationId {
        &self.id
    }

    pub fn seen(&self) -> bool {
        self.seen
    }

    pub fn kind(&self) -> &NotificationKindItem {
        &self.kind
    }

    pub fn timestamp(&self) -> &Timestamp {
        &self.timestamp
    }
}

impl TableItem<7> for NotificationItem {
    fn row(&self, _theme: &Theme, highlight: bool) -> [Cell; 7] {
        let seen = if self.seen {
            label::blank()
        } else {
            label::positive(" ● ")
        };

        let (type_name, summary, status, id) = match &self.kind() {
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

        let timestamp = if highlight {
            label::reversed(&format::timestamp(&self.timestamp))
        } else {
            label::timestamp(&format::timestamp(&self.timestamp))
        };

        [
            label::default(&format!(" {}", &self.id)).into(),
            seen.into(),
            label::alias(&type_name).into(),
            label::default(summary).into(),
            label::id(&id).into(),
            label::default(status).into(),
            timestamp.into(),
        ]
    }
}

impl TryFrom<(&Repository, Notification)> for NotificationItem {
    type Error = anyhow::Error;

    fn try_from(value: (&Repository, Notification)) -> Result<Self, Self::Error> {
        let (repo, notification) = value;
        let kind = NotificationKindItem::try_from((repo, notification.kind, notification.update))?;

        Ok(NotificationItem {
            id: notification.id,
            seen: notification.status.is_read(),
            kind,
            timestamp: notification.timestamp.into(),
        })
    }
}

impl PartialEq for NotificationItem {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
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
        } => (" ● ".into(), Color::Cyan),
    }
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

pub fn format_issue_state(state: &issue::State) -> (String, Color) {
    match state {
        issue::State::Open => (" ● ".into(), Color::Green),
        issue::State::Closed { reason: _ } => (" ● ".into(), Color::Red),
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

pub fn format_assignees(assignees: &[(Did, Option<Alias>, bool)]) -> String {
    let mut output = String::new();
    let mut assignees = assignees.iter().peekable();

    while let Some((assignee, alias, is_you)) = assignees.next() {
        output.push_str(&format_author(assignee, alias, *is_you));

        if assignees.peek().is_some() {
            output.push(',');
        }
    }
    output
}
