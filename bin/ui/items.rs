pub mod notification;

use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;
use std::ops::Range;
use std::str::FromStr;

use nom::bytes::complete::{tag, take};
use nom::multi::separated_list0;
use nom::sequence::{delimited, preceded};
use nom::{IResult, Parser};

use ansi_to_tui::IntoText;

use radicle::cob::thread::{Comment, CommentId};
use radicle::cob::{CodeLocation, CodeRange, EntryId, Label, Timestamp};
use radicle::git::Oid;
use radicle::identity::Did;
use radicle::issue;
use radicle::issue::{CloseReason, Issue, IssueId};
use radicle::node::{Alias, AliasStore, NodeId};
use radicle::patch;
use radicle::patch::{Patch, PatchId, Review};
use radicle::storage::git::Repository;
use radicle::storage::WriteRepository;
use radicle::Profile;

use radicle_surf::diff;
use radicle_surf::diff::{Hunk, Modification};

use radicle_cli::git::unified_diff::{Decode, HunkHeader};
use radicle_cli::terminal;
use radicle_cli::terminal::highlight::Highlighter;

use ratatui::prelude::*;
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::Cell;

use tui_tree_widget::TreeItem;

use radicle_tui as tui;

use tui::ui::utils::{LineMerger, MergeLocation};
use tui::ui::{span, Column};
use tui::ui::{ToRow, ToTree};

use crate::git::{Blobs, DiffStats, HunkDiff, HunkState, HunkStats};
use crate::ui;

use super::super::git;
use super::format;

pub mod filter {
    use std::fmt::{self, Write};
    use std::str::FromStr;

    use nom::bytes::complete::{tag_no_case, take};
    use nom::character::complete::{char, multispace0};
    use nom::combinator::map;
    use nom::multi::separated_list1;
    use nom::sequence::{delimited, tuple};
    use nom::IResult;

    use radicle::prelude::Did;

    /// A generic filter that needs be implemented for item filters in order to
    /// apply it.
    pub trait Filter<T> {
        fn matches(&self, item: &T) -> bool;
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum DidFilter {
        Single(Did),
        Or(Vec<Did>),
    }

    impl fmt::Display for DidFilter {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                DidFilter::Single(did) => write!(f, "{did}")?,
                DidFilter::Or(dids) => {
                    let mut it = dids.iter().peekable();
                    f.write_char('(')?;
                    while let Some(did) = it.next() {
                        write!(f, "{did}")?;
                        if it.peek().is_none() {
                            write!(f, " or ")?;
                        }
                    }
                    f.write_char(')')?;
                }
            }
            Ok(())
        }
    }

    fn parse_did(input: &str) -> IResult<&str, Did> {
        match Did::from_str(input) {
            Ok(did) => IResult::Ok(("", did)),
            Err(_) => IResult::Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Verify,
            ))),
        }
    }

    pub fn parse_did_single(input: &str) -> IResult<&str, DidFilter> {
        map(parse_did, DidFilter::Single)(input)
    }

    pub fn parse_did_or(input: &str) -> IResult<&str, DidFilter> {
        map(
            delimited(
                tuple((multispace0, char('('), multispace0)),
                separated_list1(
                    delimited(multispace0, tag_no_case("or"), multispace0),
                    take(56_usize),
                ),
                tuple((multispace0, char(')'), multispace0)),
            ),
            |dids: Vec<&str>| {
                DidFilter::Or(
                    dids.iter()
                        .filter_map(|did| Did::from_str(did).ok())
                        .collect::<Vec<_>>(),
                )
            },
        )(input)
    }
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

impl filter::Filter<IssueItem> for IssueItemFilter {
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

        let matches_authors = if !self.authors.is_empty() {
            {
                self.authors
                    .iter()
                    .any(|other| issue.author.nid == Some(**other))
            }
        } else {
            true
        };

        let matches_assigned = if self.assigned {
            issue.assignees.iter().any(|assignee| assignee.you)
        } else {
            true
        };

        let matches_assignees = if !self.assignees.is_empty() {
            {
                self.assignees.iter().any(|other| {
                    issue
                        .assignees
                        .iter()
                        .filter_map(|author| author.nid)
                        .collect::<Vec<_>>()
                        .contains(other)
                })
            }
        } else {
            true
        };

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
    pub fn is_default(&self) -> bool {
        *self == PatchItemFilter::default()
    }
}

impl filter::Filter<PatchItem> for PatchItemFilter {
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

        let matches_authors = if !self.authors.is_empty() {
            {
                self.authors
                    .iter()
                    .any(|other| patch.author.nid == Some(**other))
            }
        } else {
            true
        };

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
        let children = self.replies.iter().flat_map(CommentItem::rows).collect();

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

impl From<TermLine> for Line<'_> {
    fn from(val: TermLine) -> Self {
        Line::raw(val.0.to_string())
    }
}

/// Represents the old and new ranges of a unified diff.
pub struct DiffLineRanges {
    old: Range<u32>,
    new: Range<u32>,
}

impl From<&Hunk<Modification>> for DiffLineRanges {
    fn from(hunk: &Hunk<Modification>) -> Self {
        Self {
            old: hunk.old.clone(),
            new: hunk.new.clone(),
        }
    }
}

/// Identifies a line in a unified diff by its old and new line number.
#[derive(Clone, Debug, Default, Hash, Eq, PartialEq)]
pub struct DiffLineIndex {
    old: Option<u32>,
    new: Option<u32>,
}

impl DiffLineIndex {
    pub fn is_start_of(&self, ranges: &DiffLineRanges) -> bool {
        // TODO(erikli): Find out, why comments inserted right before or after
        // the hunk header can have such weird values.
        let old = self
            .old
            .map(|o| self.new.is_none() && o >= 4294967294)
            .unwrap_or_default();
        let new = self
            .new
            .map(|n| n == u32::MAX.saturating_sub(1) || n == ranges.new.end)
            .unwrap_or_default();

        old || new
    }

    pub fn is_end_of(&self, ranges: &DiffLineRanges) -> bool {
        let old = self
            .old
            .map(|o| o == ranges.old.end.saturating_sub(1))
            .unwrap_or_default();
        let new = self
            .new
            .map(|n| n == ranges.new.end.saturating_sub(1))
            .unwrap_or_default();

        old || new
    }

    pub fn is_inside_of(&self, ranges: &DiffLineRanges) -> bool {
        let old = self
            .old
            .map(|o| o >= ranges.old.start && o < ranges.old.end.saturating_sub(1))
            .unwrap_or_default();
        let new = self
            .new
            .map(|n| n >= ranges.new.start && n < ranges.new.end.saturating_sub(1))
            .unwrap_or_default();

        old || new
    }
}

/// Mention hunk header
impl From<&CodeLocation> for DiffLineIndex {
    fn from(location: &CodeLocation) -> Self {
        Self {
            old: location.old.as_ref().map(|r| match r {
                CodeRange::Lines { range } => range.end.saturating_sub(1) as u32,
                CodeRange::Chars { line, range: _ } => line.saturating_sub(1) as u32,
            }),
            new: location.new.as_ref().map(|r| match r {
                CodeRange::Lines { range } => range.end.saturating_sub(1) as u32,
                CodeRange::Chars { line, range: _ } => line.saturating_sub(1) as u32,
            }),
        }
    }
}

/// A type that can map a line index to a line number in a unified diff.
#[derive(Debug)]
pub struct IndexedDiffLines {
    lines: HashMap<DiffLineIndex, u32>,
}

impl IndexedDiffLines {
    pub fn new(diff: &HunkDiff) -> Self {
        let mut indexed = HashMap::new();

        if let Some(hunk) = diff.hunk() {
            for (index, line) in hunk.lines.iter().enumerate() {
                let line_index = match line {
                    Modification::Addition(addition) => DiffLineIndex {
                        old: None,
                        new: Some(addition.line_no),
                    },
                    Modification::Deletion(deletion) => DiffLineIndex {
                        old: Some(deletion.line_no),
                        new: None,
                    },
                    Modification::Context {
                        line: _,
                        line_no_old,
                        line_no_new,
                    } => DiffLineIndex {
                        old: Some(*line_no_old),
                        new: Some(*line_no_new),
                    },
                };

                indexed.insert(line_index, index as u32);
            }
        }

        Self { lines: indexed }
    }

    pub fn line(&self, index: DiffLineIndex) -> Option<u32> {
        self.lines.get(&index).copied()
    }
}

/// All comments per hunk, indexed by their merge location: start, line or end.
#[derive(Clone, Debug)]
pub struct HunkComments {
    /// All comments. Can be unsorted.
    comments: HashMap<MergeLocation, Vec<(EntryId, Comment<CodeLocation>)>>,
}

impl HunkComments {
    pub fn new(diff: &HunkDiff, comments: Vec<(EntryId, Comment<CodeLocation>)>) -> Self {
        let mut line_comments: HashMap<MergeLocation, Vec<(EntryId, Comment<CodeLocation>)>> =
            HashMap::new();
        let indexed = IndexedDiffLines::new(diff);

        for comment in comments {
            let line = if let Some(location) = comment.1.location() {
                if let Some(hunk) = diff.hunk() {
                    let ranges = DiffLineRanges::from(hunk);
                    let index = DiffLineIndex::from(location);

                    if index.is_start_of(&ranges) {
                        MergeLocation::Start
                    } else if index.is_end_of(&ranges) {
                        MergeLocation::End
                    } else {
                        let mut line = indexed
                            .line(index.clone())
                            .map(|line| MergeLocation::Line(line as usize));

                        // TODO(erikli): Properly fix index lookup rules for addition:
                        // old line number need to be ignored.
                        if line.is_none() {
                            line = indexed
                                .line(DiffLineIndex { old: None, ..index })
                                .map(|line| MergeLocation::Line(line as usize))
                        }
                        line.unwrap_or_default()
                    }
                } else {
                    MergeLocation::Unknown
                }
            } else {
                MergeLocation::Unknown
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

    pub fn all(&self) -> &HashMap<MergeLocation, Vec<(EntryId, Comment<CodeLocation>)>> {
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

/// A [`HunkItem`] that can be rendered. Hunk items are indexed sequentially and
/// provide access to the underlying hunk type.
#[derive(Clone)]
pub struct HunkItem<'a> {
    /// The underlying hunk type and its current state (accepted / rejected).
    pub diff: HunkDiff,
    /// Raw or highlighted hunk lines. Highlighting is expensive and needs to be asynchronously.
    /// Therefor, a hunks' lines need to stored separately.
    pub lines: Blobs<Vec<Line<'a>>>,
    /// A hunks' comments, indexed by line.
    pub comments: HunkComments,
}

impl From<(&Repository, &Review, &HunkDiff)> for HunkItem<'_> {
    fn from(value: (&Repository, &Review, &HunkDiff)) -> Self {
        let (repo, review, item) = value;
        let hi = Highlighter::default();

        // TODO(erikli): Start with raw, non-highlighted lines and
        // move highlighting to separate task / thread, e.g. here:
        // `let lines = blobs.raw()`
        let blobs = item.clone().blobs(repo.raw());
        let lines = blobs.highlight(hi);

        // Filter comments and include them, if:
        // - comment has a code location
        // - comment path matches hunk path
        // - comment code location is inside hunk code range
        let comments = review
            .comments()
            .filter(|(_, comment)| {
                if let Some(location) = comment.location() {
                    if location.path == *item.path() {
                        if let Some(hunk) = item.hunk() {
                            let ranges = DiffLineRanges::from(hunk);
                            let index = DiffLineIndex::from(location);

                            log::warn!("Checking comment {comment:?} at {index:?}");

                            return index.is_start_of(&ranges)
                                || index.is_inside_of(&ranges)
                                || index.is_end_of(&ranges);
                        } else {
                            return true;
                        }
                    }
                }
                false
            })
            .map(|(id, comment)| (*id, comment.clone()))
            .collect::<Vec<_>>();

        Self {
            diff: item.clone(),
            lines,
            comments: HunkComments::new(item, comments),
        }
    }
}

impl ToRow<3> for StatefulHunkItem<'_> {
    fn to_row(&self) -> [Cell; 3] {
        let build_stats_spans = |stats: &DiffStats| -> Vec<Span<'_>> {
            let mut cell = vec![];
            let comments = &self.inner().comments;

            if !comments.is_empty() {
                cell.push(
                    span::default(&format!(" {} ", comments.len()))
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
                cell.push(span::default(&format!("+{added}")).light_green().dim());
            }

            if added > 0 && deleted > 0 {
                cell.push(span::default(",").dim());
            }

            if deleted > 0 {
                cell.push(span::default(&format!("-{deleted}")).light_red().dim());
            }

            cell
        };

        match &self.inner().diff {
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
                    Line::from(ui::span::hunk_state(self.state()))
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
                    Line::from(ui::span::hunk_state(self.state()))
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
                    Line::from(ui::span::hunk_state(self.state()))
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
                    Line::from(ui::span::hunk_state(self.state()))
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
                    Line::from(ui::span::hunk_state(self.state()))
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
                Line::from(ui::span::hunk_state(self.state()))
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
                Line::from(ui::span::hunk_state(self.state()))
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
                span::default(&format!(" {count} comments "))
                    .dim()
                    .reversed()
            }
        } else {
            span::blank()
        };

        match &self.diff {
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
        match &self.diff {
            HunkDiff::Added { hunk, .. }
            | HunkDiff::Modified { hunk, .. }
            | HunkDiff::Deleted { hunk, .. } => {
                let mut lines = hunk
                    .as_ref()
                    .map(|hunk| Text::from(hunk.to_text(&self.lines)));

                lines = lines.map(|lines| {
                    let divider = span::default(&"─".to_string().repeat(500)).gray().dim();

                    let mut merge = HashMap::new();
                    for (line, comments) in self.comments.all() {
                        merge.insert(
                            line.clone(),
                            comments
                                .iter()
                                .enumerate()
                                .map(|(idx, comment)| {
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
                                    rendered.extend(comment.1.body().lines().map(|line| {
                                        Line::from([span::default(line).gray()].to_vec())
                                    }));

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
                    let merged = LineMerger::new(lines.lines).merge(merge, None);
                    Text::from(merged)
                });

                lines
            }
            _ => None,
        }
    }
}

impl Debug for HunkItem<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HunkItem")
            .field("inner", &self.diff)
            .field("comments", &self.comments)
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct StatefulHunkItem<'a>(HunkItem<'a>, HunkState);

impl<'a> StatefulHunkItem<'a> {
    pub fn new(inner: HunkItem<'a>, state: HunkState) -> Self {
        Self(inner, state)
    }

    pub fn inner(&self) -> &HunkItem<'a> {
        &self.0
    }

    pub fn state(&self) -> &HunkState {
        &self.1
    }

    pub fn update_state(&mut self, state: &HunkState) {
        self.1 = state.clone();
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
    use std::path::PathBuf;

    use anyhow::Result;

    use crate::test;

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
    fn diff_line_index_checks_ranges_correctly() -> Result<()> {
        let commit = Oid::from_str("a32c4b93e2573fd83b15ac1ad6bf1317dc8fd760").unwrap();
        let path = PathBuf::from_str("main.rs").unwrap();

        // --------------------------------------------------------------------
        // At the top.
        // --------------------------------------------------------------------
        // @@ -3,8 +3,7 @@
        // 3   3     // or if you prefer to use your keyboard, you can use the "Ctrl + Enter"
        // 4   4     // shortcut.
        // 5   5
        // 6       - // This code is editable, feel free to hack it!
        // 7       - // You can always return to the original code by clicking the "Reset" button ->
        //     6   + // This is still a comment.
        // --------------------------------------------------------------------
        // In the middle.
        // --------------------------------------------------------------------
        // 8   7
        // 9   8     // This is the main function.
        // 10  9     fn main() {
        // ---------------------------------------------------------------------
        // At the end.
        // ---------------------------------------------------------------------
        let diff = test::fixtures::simple_modified_hunk_diff(&path, commit)?;
        let ranges = DiffLineRanges::from(diff.hunk().unwrap());

        let start = CodeLocation {
            commit,
            path: path.clone(),
            old: Some(CodeRange::Lines { range: 3..12 }),
            new: Some(CodeRange::Lines { range: 3..11 }),
        };
        assert!(DiffLineIndex::from(&start).is_start_of(&ranges));
        assert!(!DiffLineIndex::from(&start).is_inside_of(&ranges));
        assert!(!DiffLineIndex::from(&start).is_end_of(&ranges));

        let inside = CodeLocation {
            commit,
            path: path.clone(),
            old: Some(CodeRange::Lines { range: 3..8 }),
            new: Some(CodeRange::Lines { range: 3..7 }),
        };
        assert!(DiffLineIndex::from(&inside).is_inside_of(&ranges));
        assert!(!DiffLineIndex::from(&inside).is_start_of(&ranges));
        assert!(!DiffLineIndex::from(&inside).is_end_of(&ranges));

        let end = CodeLocation {
            commit,
            path: path.clone(),
            old: Some(CodeRange::Lines { range: 3..11 }),
            new: Some(CodeRange::Lines { range: 3..10 }),
        };
        assert!(DiffLineIndex::from(&end).is_end_of(&ranges));
        assert!(!DiffLineIndex::from(&end).is_start_of(&ranges));
        assert!(!DiffLineIndex::from(&end).is_inside_of(&ranges));

        let outside = CodeLocation {
            commit,
            path: path.clone(),
            old: Some(CodeRange::Lines { range: 125..127 }),
            new: Some(CodeRange::Lines { range: 125..128 }),
        };
        assert!(!DiffLineIndex::from(&outside).is_start_of(&ranges));
        assert!(!DiffLineIndex::from(&outside).is_inside_of(&ranges));
        assert!(!DiffLineIndex::from(&outside).is_end_of(&ranges));

        Ok(())
    }

    #[test]
    fn hunk_comments_on_modified_simple_are_inserted_correctly() -> Result<()> {
        let alice = test::fixtures::node_with_repo();

        let commit = Oid::from_str("a32c4b93e2573fd83b15ac1ad6bf1317dc8fd760").unwrap();
        let path = PathBuf::from_str("main.rs").unwrap();

        // --------------------------------------------------------------------
        // At the top.
        // --------------------------------------------------------------------
        // @@ -3,8 +3,7 @@
        // 3   3     // or if you prefer to use your keyboard, you can use the "Ctrl + Enter"
        // 4   4     // shortcut.
        // 5   5
        // 6       - // This code is editable, feel free to hack it!
        // 7       - // You can always return to the original code by clicking the "Reset" button ->
        //     6   + // This is still a comment.
        // --------------------------------------------------------------------
        // In the middle.
        // --------------------------------------------------------------------
        // 8   7
        // 9   8     // This is the main function.
        // 10  9     fn main() {
        // ---------------------------------------------------------------------
        // At the end.
        // ---------------------------------------------------------------------
        let diff = test::fixtures::simple_modified_hunk_diff(&path, commit)?;

        let top = (
            Oid::from_str("05ac6202655dcde6c2613702fec07c2e2fe8f382").unwrap(),
            Comment::new(
                *alice.node.signer.public_key(),
                "At the top.".to_string(),
                None,
                Some(CodeLocation {
                    commit,
                    path: path.clone(),
                    old: Some(CodeRange::Lines { range: 3..12 }),
                    new: Some(CodeRange::Lines { range: 3..11 }),
                }),
                vec![],
                Timestamp::from_secs(0),
            ),
        );
        let middle = (
            Oid::from_str("2d09104bf2d6ad328aa72594b679d2d6c5a61865").unwrap(),
            Comment::new(
                *alice.node.signer.public_key(),
                "In the middle.".to_string(),
                None,
                Some(CodeLocation {
                    commit,
                    path: path.clone(),
                    old: Some(CodeRange::Lines { range: 3..8 }),
                    new: Some(CodeRange::Lines { range: 3..7 }),
                }),
                vec![],
                Timestamp::from_secs(0),
            ),
        );
        let end = (
            Oid::from_str("8280317b308ba1bf2cef04533efb15d920431e86").unwrap(),
            Comment::new(
                *alice.node.signer.public_key(),
                "At the end.".to_string(),
                None,
                Some(CodeLocation {
                    commit,
                    path: path.clone(),
                    old: Some(CodeRange::Lines { range: 3..11 }),
                    new: Some(CodeRange::Lines { range: 3..10 }),
                }),
                vec![],
                Timestamp::from_secs(0),
            ),
        );

        let comments = {
            let comments = [top.clone(), middle.clone(), end.clone()];
            HunkComments::new(&diff, comments.to_vec())
        };

        for expected in [
            (top, MergeLocation::Start),
            (middle, MergeLocation::Line(5)),
            (end, MergeLocation::End),
        ] {
            let (line, expected) = (expected.1, expected.0);
            let actual = comments.all().get(&line);
            assert_ne!(actual, None, "No comment found at {line:?}");

            let actual = actual.unwrap().first().unwrap();
            assert_eq!(actual.0, expected.0);
        }

        Ok(())
    }

    #[test]
    fn hunk_comments_on_modified_complex_are_inserted_correctly() -> Result<()> {
        let alice = test::fixtures::node_with_repo();

        let commit = Oid::from_str("a32c4b93e2573fd83b15ac1ad6bf1317dc8fd760").unwrap();
        let path = PathBuf::from_str("main.rs").unwrap();

        // --------------------------------------------------------------------
        // At the top.
        // --------------------------------------------------------------------
        // @@ -1,17 +1,15 @@
        // 1       - use radicle::issue::IssueId;
        // 2       - use tui::ui::state::ItemState;
        // 3       - use tui::SelectionExit;
        // --------------------------------------------------------------------
        // After deletion.
        // --------------------------------------------------------------------
        // 4   1     use tuirealm::command::{Cmd, CmdResult, Direction as MoveDirection};
        // 5   2     use tuirealm::event::{Event, Key, KeyEvent};
        // 6   3     use tuirealm::{MockComponent, NoUserEvent};
        // 7   4
        // 8   5     use radicle_tui as tui;
        // 9   6
        //     7   + use tui::ui::state::ItemState;
        // 10  8     use tui::ui::widget::container::{AppHeader, GlobalListener, LabeledContainer};
        // 11  9     use tui::ui::widget::context::{ContextBar, Shortcuts};
        // 12  10    use tui::ui::widget::list::PropertyList;
        // 13      -
        // 14  11    use tui::ui::widget::Widget;
        //     12  + use tui::{Id, SelectionExit};
        // 15  13
        // 16  14    use super::ui::{IdSelect, OperationSelect};
        // --------------------------------------------------------------------
        // Before last line.
        // --------------------------------------------------------------------
        // 17  15    use super::{IssueOperation, Message};
        let diff = test::fixtures::complex_modified_hunk_diff(&path, commit)?;

        let top = (
            Oid::from_str("05ac6202655dcde6c2613702fec07c2e2fe8f382").unwrap(),
            Comment::new(
                *alice.node.signer.public_key(),
                "At the top.".to_string(),
                None,
                Some(CodeLocation {
                    commit,
                    path: path.clone(),
                    old: Some(CodeRange::Lines { range: 1..18 }),
                    new: Some(CodeRange::Lines { range: 1..17 }),
                }),
                vec![],
                Timestamp::from_secs(0),
            ),
        );
        let after_deletion = (
            Oid::from_str("2d09104bf2d6ad328aa72594b679d2d6c5a61865").unwrap(),
            Comment::new(
                *alice.node.signer.public_key(),
                "After deletion.".to_string(),
                None,
                Some(CodeLocation {
                    commit,
                    path: path.clone(),
                    old: Some(CodeRange::Lines { range: 1..4 }),
                    new: None,
                }),
                vec![],
                Timestamp::from_secs(0),
            ),
        );
        let before_last_line = (
            Oid::from_str("60972bca0c9e686e76b0a5123acb3c8c60c38b1e").unwrap(),
            Comment::new(
                *alice.node.signer.public_key(),
                "Before last line".to_string(),
                None,
                Some(CodeLocation {
                    commit,
                    path: path.clone(),
                    old: Some(CodeRange::Lines { range: 1..17 }),
                    new: Some(CodeRange::Lines { range: 1..15 }),
                }),
                vec![],
                Timestamp::from_secs(0),
            ),
        );

        let comments = {
            let comments = [
                top.clone(),
                after_deletion.clone(),
                before_last_line.clone(),
            ];
            HunkComments::new(&diff, comments.to_vec())
        };

        for expected in [
            (top, MergeLocation::Start),
            (after_deletion, MergeLocation::Line(2)),
            (before_last_line, MergeLocation::Line(17)),
        ] {
            let (line, expected) = (expected.1, expected.0);
            let actual = comments.all().get(&line);
            assert_ne!(actual, None, "No comment found at {line:?}");

            let actual = actual.unwrap().first().unwrap();
            assert_eq!(actual.0, expected.0);
        }

        Ok(())
    }

    #[test]
    fn hunk_comments_on_deleted_simple_are_inserted_correctly() -> Result<()> {
        let alice = test::fixtures::node_with_repo();

        let commit = Oid::from_str("a32c4b93e2573fd83b15ac1ad6bf1317dc8fd760").unwrap();
        let path = PathBuf::from_str("README.md").unwrap();

        // --------------------------------------------------------------------
        // At the top.
        // --------------------------------------------------------------------
        // @@ -1,1 +0,0 @@
        //  -TBD
        // --------------------------------------------------------------------
        // At the end.
        // --------------------------------------------------------------------
        let diff = test::fixtures::deleted_hunk_diff(&path, commit)?;

        let top = (
            Oid::from_str("05ac6202655dcde6c2613702fec07c2e2fe8f382").unwrap(),
            Comment::new(
                *alice.node.signer.public_key(),
                "At the top.".to_string(),
                None,
                Some(CodeLocation {
                    commit,
                    path: path.clone(),
                    old: Some(CodeRange::Lines { range: 1..3 }),
                    new: Some(CodeRange::Lines { range: 0..1 }),
                }),
                vec![],
                Timestamp::from_secs(0),
            ),
        );
        let end = (
            Oid::from_str("8280317b308ba1bf2cef04533efb15d920431e86").unwrap(),
            Comment::new(
                *alice.node.signer.public_key(),
                "At the end.".to_string(),
                None,
                Some(CodeLocation {
                    commit,
                    path: path.clone(),
                    old: Some(CodeRange::Lines { range: 1..2 }),
                    new: None,
                }),
                vec![],
                Timestamp::from_secs(0),
            ),
        );

        let comments = {
            let comments = [top.clone(), end.clone()];
            HunkComments::new(&diff, comments.to_vec())
        };

        for expected in [(top, MergeLocation::Start), (end, MergeLocation::End)] {
            let (line, expected) = (expected.1, expected.0);
            let actual = comments.all().get(&line);
            assert_ne!(actual, None, "No comment found at {line:?}");

            let actual = actual.unwrap().first().unwrap();
            assert_eq!(actual.0, expected.0);
        }

        Ok(())
    }
}
