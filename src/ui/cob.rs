pub mod format;

use radicle_surf;

use tuirealm::props::{Color, Style};
use tuirealm::tui::text::Line;
use tuirealm::tui::widgets::Cell;

use radicle::node::{Alias, AliasStore};

use radicle::prelude::Did;
use radicle::storage::git::Repository;
use radicle::storage::{Oid, ReadRepository};
use radicle::Profile;

use radicle::cob::issue::{self, Issue, IssueId};
use radicle::cob::patch::{self, Patch, PatchId};
use radicle::cob::{Label, Timestamp};

use crate::ui::theme::Theme;
use crate::ui::widget::list::{ListItem, TableItem};

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
