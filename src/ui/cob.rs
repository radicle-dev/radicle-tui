pub mod format;

use radicle_surf;

use tuirealm::props::{Color, Style, TextModifiers};
use tuirealm::tui::text::{Span, Spans};
use tuirealm::tui::widgets::Cell;

use radicle::node::{Alias, AliasStore};

use radicle::prelude::Did;
use radicle::storage::git::Repository;
use radicle::storage::{Oid, ReadRepository};
use radicle::Profile;

use radicle::cob::issue::{Issue, IssueId, State as IssueState};
use radicle::cob::patch::{Patch, PatchId, State as PatchState};
use radicle::cob::{Label, Timestamp};

use crate::ui::theme::Theme;
use crate::ui::widget::list::{ListItem, TableItem};

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
    state: PatchState,
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

    pub fn state(&self) -> &PatchState {
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
    fn row(&self, theme: &Theme) -> [Cell; 8] {
        let (icon, color) = format_patch_state(&self.state);
        let state = Cell::from(icon).style(Style::default().fg(color));

        let id = Cell::from(format::cob(&self.id))
            .style(Style::default().fg(theme.colors.browser_list_id));

        let title = Cell::from(self.title.clone())
            .style(Style::default().fg(theme.colors.browser_list_title));

        let author_style = match &self.author.alias {
            Some(_) => Style::default().fg(theme.colors.browser_list_author),
            None => Style::default()
                .fg(theme.colors.browser_list_author)
                .add_modifier(TextModifiers::DIM),
        };
        let author = Cell::from(format_author(
            &self.author.did,
            &self.author.alias,
            self.author.is_you,
        ))
        .style(author_style);

        let head = Cell::from(format::oid(self.head))
            .style(Style::default().fg(theme.colors.browser_patch_list_head));

        let added = Cell::from(format!("+{}", self.added))
            .style(Style::default().fg(theme.colors.browser_patch_list_added));

        let removed = Cell::from(format!("-{}", self.removed))
            .style(Style::default().fg(theme.colors.browser_patch_list_removed));

        let updated = Cell::from(format::timestamp(&self.timestamp))
            .style(Style::default().fg(theme.colors.browser_list_timestamp));

        [state, id, title, author, head, added, removed, updated]
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
    state: IssueState,
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

    pub fn state(&self) -> &IssueState {
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
    fn row(&self, theme: &Theme) -> [Cell; 7] {
        let (icon, color) = format_issue_state(&self.state);
        let state = Cell::from(icon).style(Style::default().fg(color));

        let id = Cell::from(format::cob(&self.id))
            .style(Style::default().fg(theme.colors.browser_list_id));

        let title = Cell::from(self.title.clone())
            .style(Style::default().fg(theme.colors.browser_list_title));

        let author_style = match &self.author.alias {
            Some(_) => Style::default().fg(theme.colors.browser_list_author),
            None => Style::default()
                .fg(theme.colors.browser_list_author)
                .add_modifier(TextModifiers::DIM),
        };
        let author = Cell::from(format_author(
            &self.author.did,
            &self.author.alias,
            self.author.is_you,
        ))
        .style(author_style);

        let labels = Cell::from(format_labels(&self.labels))
            .style(Style::default().fg(theme.colors.browser_list_labels));

        let assignees = self
            .assignees
            .iter()
            .map(|author| (author.did, author.alias.clone(), author.is_you))
            .collect::<Vec<_>>();
        let assignees = Cell::from(format_assignees(&assignees))
            .style(Style::default().fg(theme.colors.browser_list_author));

        let opened = Cell::from(format::timestamp(&self.timestamp))
            .style(Style::default().fg(theme.colors.browser_list_timestamp));

        [state, id, title, author, labels, assignees, opened]
    }
}

impl ListItem for IssueItem {
    fn row(&self, theme: &Theme) -> tuirealm::tui::widgets::ListItem {
        let (state, state_color) = format_issue_state(&self.state);
        let author_style = match &self.author.alias {
            Some(_) => Style::default().fg(theme.colors.browser_list_author),
            None => Style::default()
                .fg(theme.colors.browser_list_author)
                .add_modifier(TextModifiers::DIM),
        };
        let lines = vec![
            Spans::from(vec![
                Span::styled(state, Style::default().fg(state_color)),
                Span::styled(
                    self.title.clone(),
                    Style::default().fg(theme.colors.browser_list_title),
                ),
            ]),
            Spans::from(vec![
                Span::raw(String::from("   ")),
                Span::styled(
                    format_author(&self.author.did, &self.author.alias, self.author.is_you),
                    author_style,
                ),
                Span::styled(
                    format!(" {} ", theme.icons.property_divider),
                    Style::default().fg(theme.colors.property_divider_fg),
                ),
                Span::styled(
                    format::timestamp(&self.timestamp),
                    Style::default().fg(theme.colors.browser_list_timestamp),
                ),
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

pub fn format_patch_state(state: &PatchState) -> (String, Color) {
    match state {
        PatchState::Open { conflicts: _ } => (" ● ".into(), Color::Green),
        PatchState::Archived => (" ● ".into(), Color::Yellow),
        PatchState::Draft => (" ● ".into(), Color::Gray),
        PatchState::Merged {
            revision: _,
            commit: _,
        } => (" ✔ ".into(), Color::Blue),
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

pub fn format_issue_state(state: &IssueState) -> (String, Color) {
    match state {
        IssueState::Open => (" ● ".into(), Color::Green),
        IssueState::Closed { reason: _ } => (" ● ".into(), Color::Red),
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
