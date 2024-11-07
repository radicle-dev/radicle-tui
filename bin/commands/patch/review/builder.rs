//! Review builder.
//!
//! This module enables a user to review a patch by interactively viewing and accepting diff hunks.
//! The interaction and output is modeled around `git add -p`.
//!
//! To implement this behavior, we keep a hidden Git tree object that tracks the state of the
//! repository including the accepted hunks. Thus, every time a diff hunk is accepted, it is applied
//! to that tree. We call that tree the "brain", as it tracks what the code reviewer has reviewed.
//!
//! The brain starts out equalling the tree of the base branch, and eventually, when the brain
//! matches the tree of the patch being reviewed (by accepting hunks), we can say that the patch has
//! been fully reviewed.
//!
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::ops::{Deref, Not, Range};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{fmt, io};

use radicle::cob;
use radicle::cob::cache::NoCache;
use radicle::cob::patch::{PatchId, Revision, Verdict};
use radicle::cob::{CodeLocation, CodeRange};
use radicle::git;
use radicle::git::Oid;
use radicle::patch::PatchMut;
use radicle::prelude::*;
use radicle::storage::git::{cob::DraftStore, Repository};
use radicle_surf::diff::*;
use radicle_term::{Element, VStack};

use radicle_cli::git::pretty_diff::ToPretty;
use radicle_cli::git::pretty_diff::{Blob, Blobs, Repo};
use radicle_cli::git::unified_diff::{self, FileHeader};
use radicle_cli::git::unified_diff::{Encode, HunkHeader};
use radicle_cli::terminal as term;
use radicle_cli::terminal::highlight::Highlighter;

use crate::cob::HunkItem;

/// Help message shown to user.
const HELP: &str = "\
y - accept this hunk
n - ignore this hunk
c - comment on this hunk
j - leave this hunk undecided, see next hunk
k - leave this hunk undecided, see previous hunk
s - split the current hunk into smaller hunks
q - quit; do not accept this hunk nor any of the remaining ones
? - print help";

/// A terminal or file where the review UI output can be written to.
trait PromptWriter: io::Write {
    /// Is the writer a terminal?
    fn is_terminal(&self) -> bool;
}

impl PromptWriter for Box<dyn PromptWriter> {
    fn is_terminal(&self) -> bool {
        self.deref().is_terminal()
    }
}

impl<T: io::Write + io::IsTerminal> PromptWriter for T {
    fn is_terminal(&self) -> bool {
        <Self as io::IsTerminal>::is_terminal(self)
    }
}

/// The actions that a user can carry out on a review item.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReviewAction {
    Accept,
    Ignore,
    Comment,
    Split,
    Next,
    Previous,
    Help,
    Quit,
}

impl ReviewAction {
    /// Ask the user what action to take.
    fn prompt(
        mut input: impl io::BufRead,
        mut output: impl io::Write,
        prompt: impl fmt::Display,
    ) -> io::Result<Option<Self>> {
        write!(&mut output, "{prompt} ")?;

        let mut s = String::new();
        input.read_line(&mut s)?;

        if s.trim().is_empty() {
            return Ok(None);
        }
        Self::from_str(s.trim())
            .map(Some)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))
    }
}

impl std::fmt::Display for ReviewAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Accept => write!(f, "y"),
            Self::Ignore => write!(f, "n"),
            Self::Comment => write!(f, "c"),
            Self::Split => write!(f, "s"),
            Self::Next => write!(f, "j"),
            Self::Previous => write!(f, "k"),
            Self::Help => write!(f, "?"),
            Self::Quit => write!(f, "q"),
        }
    }
}

impl FromStr for ReviewAction {
    type Err = io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "y" => Ok(Self::Accept),
            "n" => Ok(Self::Ignore),
            "c" => Ok(Self::Comment),
            "s" => Ok(Self::Split),
            "j" => Ok(Self::Next),
            "k" => Ok(Self::Previous),
            "?" => Ok(Self::Help),
            "q" => Ok(Self::Quit),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid action '{s}'"),
            )),
        }
    }
}

/// Queue of items (usually hunks) left to review.
#[derive(Clone, Default)]
pub struct ReviewQueue {
    /// Hunks left to review.
    queue: VecDeque<(usize, HunkItem)>,
}

impl ReviewQueue {
    /// Add a file to the queue.
    /// Mostly splits files into individual review items (eg. hunks) to review.
    fn add_file(&mut self, file: FileDiff) {
        match file {
            FileDiff::Moved(moved) => {
                self.add_item(HunkItem::FileMoved { moved });
            }
            FileDiff::Copied(copied) => {
                self.add_item(HunkItem::FileCopied { copied });
            }
            FileDiff::Added(a) => {
                self.add_item(HunkItem::FileAdded {
                    path: a.path,
                    new: a.new,
                    hunk: if let DiffContent::Plain {
                        hunks: Hunks(mut hs),
                        ..
                    } = a.diff.clone()
                    {
                        hs.pop()
                    } else {
                        None
                    },
                    _stats: a.diff.stats().cloned(),
                });
            }
            FileDiff::Deleted(d) => {
                self.add_item(HunkItem::FileDeleted {
                    path: d.path,
                    old: d.old,
                    hunk: if let DiffContent::Plain {
                        hunks: Hunks(mut hs),
                        ..
                    } = d.diff.clone()
                    {
                        hs.pop()
                    } else {
                        None
                    },
                    _stats: d.diff.stats().cloned(),
                });
            }
            FileDiff::Modified(m) => {
                if m.old.mode != m.new.mode {
                    self.add_item(HunkItem::FileModeChanged {
                        path: m.path.clone(),
                        old: m.old.clone(),
                        new: m.new.clone(),
                    });
                }
                match m.diff {
                    DiffContent::Empty => {
                        // Likely a file mode change, which is handled above.
                    }
                    DiffContent::Binary => {
                        self.add_item(HunkItem::FileModified {
                            path: m.path.clone(),
                            old: m.old.clone(),
                            new: m.new.clone(),
                            hunk: None,
                            _stats: m.diff.stats().cloned(),
                        });
                    }
                    DiffContent::Plain {
                        hunks: Hunks(hunks),
                        eof,
                        stats,
                    } => {
                        for hunk in hunks {
                            self.add_item(HunkItem::FileModified {
                                path: m.path.clone(),
                                old: m.old.clone(),
                                new: m.new.clone(),
                                hunk: Some(hunk),
                                _stats: Some(stats),
                            });
                        }
                        if let EofNewLine::OldMissing | EofNewLine::NewMissing = eof {
                            self.add_item(HunkItem::FileEofChanged {
                                path: m.path.clone(),
                                old: m.old.clone(),
                                new: m.new.clone(),
                                _eof: eof,
                            })
                        }
                    }
                }
            }
        }
    }

    fn add_item(&mut self, item: HunkItem) {
        self.queue.push_back((self.queue.len(), item));
    }
}

impl From<Diff> for ReviewQueue {
    fn from(diff: Diff) -> Self {
        let mut queue = Self::default();
        for file in diff.into_files() {
            queue.add_file(file);
        }
        queue
    }
}

impl std::ops::Deref for ReviewQueue {
    type Target = VecDeque<(usize, HunkItem)>;

    fn deref(&self) -> &Self::Target {
        &self.queue
    }
}

impl std::ops::DerefMut for ReviewQueue {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.queue
    }
}

impl Iterator for ReviewQueue {
    type Item = (usize, HunkItem);

    fn next(&mut self) -> Option<Self::Item> {
        self.queue.pop_front()
    }
}

/// Builds a review for a single file.
/// Adjusts line deltas when a hunk is ignored.
pub struct FileReviewBuilder {
    delta: i32,
}

impl FileReviewBuilder {
    fn new(item: &HunkItem) -> Self {
        Self { delta: 0 }
    }

    fn set_item(&mut self, item: &HunkItem) -> &mut Self {
        self.delta = 0;
        self
    }

    fn ignore_item(&mut self, item: &HunkItem) {
        if let Some(h) = item.hunk_header() {
            self.delta += h.new_size as i32 - h.old_size as i32;
        }
    }
}

/// Represents the reviewer's brain, ie. what they have seen or not seen in terms
/// of changes introduced by a patch.
pub struct Brain<'a> {
    /// Where the review draft is being stored.
    refname: git::Namespaced<'a>,
    /// The commit pointed to by the ref.
    head: git::raw::Commit<'a>,
    /// The tree of accepted changes pointed to by the head commit.
    accepted: git::raw::Tree<'a>,
}

impl<'a> Brain<'a> {
    /// Create a new brain in the repository.
    pub fn new(
        patch: PatchId,
        remote: &NodeId,
        base: git::raw::Commit,
        repo: &'a git::raw::Repository,
    ) -> Result<Self, git::raw::Error> {
        let refname = Self::refname(&patch, remote);
        let author = repo.signature()?;
        let oid = repo.commit(
            Some(refname.as_str()),
            &author,
            &author,
            &format!("Review for {patch}"),
            &base.tree()?,
            // TODO: Verify this is necessary, shouldn't matter.
            &[&base],
        )?;
        let head = repo.find_commit(oid)?;
        let tree = head.tree()?;

        Ok(Self {
            refname,
            head,
            accepted: tree,
        })
    }

    /// Return the content identifier of this brain. This represents the state of the
    /// accepted hunks, ie. the git tree.
    pub fn cid(&self) -> Oid {
        self.accepted.id().into()
    }

    /// Load an existing brain from the repository.
    pub fn load(
        patch: PatchId,
        remote: &NodeId,
        repo: &'a git::raw::Repository,
    ) -> Result<Self, git::raw::Error> {
        // TODO: Validate this leads to correct UX for potentially abandoned drafts on
        // past revisions.
        let refname = Self::refname(&patch, remote);
        let head = repo.find_reference(&refname)?.peel_to_commit()?;
        let tree = head.tree()?;

        Ok(Self {
            refname,
            head,
            accepted: tree,
        })
    }

    /// Accept changes to the brain.
    pub fn accept(
        &mut self,
        diff: git::raw::Diff,
        repo: &'a git::raw::Repository,
    ) -> Result<(), git::raw::Error> {
        let mut index = repo.apply_to_tree(&self.accepted, &diff, None)?;
        let accepted = index.write_tree_to(repo)?;
        self.accepted = repo.find_tree(accepted)?;

        // Update review with new brain.
        let head = self.head.amend(
            Some(&self.refname),
            None,
            None,
            None,
            None,
            Some(&self.accepted),
        )?;
        self.head = repo.find_commit(head)?;

        Ok(())
    }

    /// Get the brain's refname given the patch and remote.
    pub fn refname(patch: &PatchId, remote: &NodeId) -> git::Namespaced<'a> {
        git::refs::storage::draft::review(remote, patch)
    }

    pub fn head(&self) -> &git::raw::Commit<'a> {
        &self.head
    }

    pub fn accepted(&self) -> &git::raw::Tree<'a> {
        &self.accepted
    }
}

/// Builds a patch review interactively, across multiple files.
pub struct ReviewBuilder<'a, G> {
    /// Patch being reviewed.
    patch_id: PatchId,
    /// Signer.
    signer: &'a G,
    /// Stored copy of repository.
    repo: &'a Repository,
    /// Verdict for review items.
    verdict: Option<Verdict>,
}

impl<'a, G: Signer> ReviewBuilder<'a, G> {
    /// Create a new review builder.
    pub fn new(patch_id: PatchId, signer: &'a G, repo: &'a Repository) -> Self {
        Self {
            patch_id,
            signer,
            repo,
            verdict: None,
        }
    }

    /// Give this verdict to all review items. Set to `None` to not give a verdict.
    pub fn verdict(mut self, verdict: Option<Verdict>) -> Self {
        self.verdict = verdict;
        self
    }

    /// Assemble the review for the given revision.
    pub fn queue(&self, brain: &'a Brain<'a>, revision: &Revision) -> anyhow::Result<ReviewQueue> {
        let repo = self.repo.raw();
        let tree = {
            let commit = repo.find_commit(revision.head().into())?;

            log::info!(
                "Loading queue patch: Patch[commit({}), tree({})], Brain[tree({})]",
                commit.id(),
                commit.tree()?.id(),
                &brain.accepted().id()
            );

            commit.tree()?
        };

        let mut opts = git::raw::DiffOptions::new();
        opts.patience(true).minimal(true).context_lines(3_u32);

        let diff = self.diff(&brain.accepted(), &tree, repo, &mut opts)?;

        Ok(ReviewQueue::from(diff))
    }

    pub fn diff(
        &self,
        brain: &git::raw::Tree<'_>,
        tree: &git::raw::Tree<'_>,
        repo: &'a git::raw::Repository,
        opts: &mut git::raw::DiffOptions,
    ) -> Result<Diff, Error> {
        let mut find_opts = git::raw::DiffFindOptions::new();
        find_opts.exact_match_only(true);
        find_opts.all(true);
        find_opts.copies(false); // We don't support finding copies at the moment.

        let mut diff = repo.diff_tree_to_tree(Some(brain), Some(tree), Some(opts))?;
        diff.find_similar(Some(&mut find_opts))?;

        let diff = Diff::try_from(diff)?;

        Ok(diff)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ReviewComment {
    pub location: CodeLocation,
    pub body: String,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Diff(#[from] unified_diff::Error),
    #[error(transparent)]
    Surf(#[from] radicle_surf::diff::git::error::Diff),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Format(#[from] std::fmt::Error),
    #[error(transparent)]
    Git(#[from] git::raw::Error),
}

#[derive(Debug)]
pub struct CommentBuilder {
    commit: Oid,
    path: PathBuf,
    comments: Vec<ReviewComment>,
}

impl CommentBuilder {
    pub fn new(commit: Oid, path: PathBuf) -> Self {
        Self {
            commit,
            path,
            comments: Vec::new(),
        }
    }

    pub fn edit(mut self, hunk: &Hunk<Modification>) -> Result<Vec<ReviewComment>, Error> {
        let mut input = String::new();
        for line in hunk.to_unified_string()?.lines() {
            writeln!(&mut input, "> {line}")?;
        }
        let output = term::Editor::new().extension("diff").edit(input)?;

        if let Some(output) = output {
            let header = HunkHeader::try_from(hunk)?;
            self.add_hunk(header, &output);
        }
        Ok(self.comments())
    }

    pub fn add_hunk(&mut self, hunk: HunkHeader, input: &str) -> &mut Self {
        let lines = input.trim().lines().map(|l| l.trim());
        let (mut old_line, mut new_line) = (hunk.old_line_no as usize, hunk.new_line_no as usize);
        let (mut old_start, mut new_start) = (old_line, new_line);
        let mut comment = String::new();

        for line in lines {
            if line.starts_with('>') {
                if !comment.is_empty() {
                    self.add_comment(
                        &hunk,
                        &comment,
                        old_start..old_line - 1,
                        new_start..new_line - 1,
                    );

                    old_start = old_line - 1;
                    new_start = new_line - 1;

                    comment.clear();
                }
                match line.trim_start_matches('>').trim_start().chars().next() {
                    Some('-') => old_line += 1,
                    Some('+') => new_line += 1,
                    _ => {
                        old_line += 1;
                        new_line += 1;
                    }
                }
            } else {
                comment.push_str(line);
                comment.push('\n');
            }
        }
        if !comment.is_empty() {
            self.add_comment(
                &hunk,
                &comment,
                old_start..old_line - 1,
                new_start..new_line - 1,
            );
        }
        self
    }

    fn add_comment(
        &mut self,
        hunk: &HunkHeader,
        comment: &str,
        mut old_range: Range<usize>,
        mut new_range: Range<usize>,
    ) {
        // Empty lines between quoted text can generate empty comments
        // that should be filtered out.
        if comment.trim().is_empty() {
            return;
        }
        // Top-level comment, it should apply to the whole hunk.
        if old_range.is_empty() && new_range.is_empty() {
            old_range = hunk.old_line_no as usize..(hunk.old_line_no + hunk.old_size + 1) as usize;
            new_range = hunk.new_line_no as usize..(hunk.new_line_no + hunk.new_size + 1) as usize;
        }
        let old_range = old_range
            .is_empty()
            .not()
            .then_some(old_range)
            .map(|range| CodeRange::Lines { range });
        let new_range = (new_range)
            .is_empty()
            .not()
            .then_some(new_range)
            .map(|range| CodeRange::Lines { range });

        self.comments.push(ReviewComment {
            location: CodeLocation {
                commit: self.commit,
                path: self.path.clone(),
                old: old_range,
                new: new_range,
            },
            body: comment.trim().to_owned(),
        });
    }

    fn comments(self) -> Vec<ReviewComment> {
        self.comments
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_review_comments_basic() {
        let input = r#"
> @@ -2559,18 +2560,18 @@ where
>                  // Only consider onion addresses if configured.
>                  AddressType::Onion => self.config.onion.is_some(),
>                  AddressType::Dns | AddressType::Ipv4 | AddressType::Ipv6 => true,
> -            })
> -            .take(wanted)
> -            .collect::<Vec<_>>(); // # -2564

Comment #1.

> +            });
>
> -        if available.len() < target {
> -            log::warn!( # -2567
> +        // Peers we are going to attempt connections to.
> +        let connect = available.take(wanted).collect::<Vec<_>>();

Comment #2.

> +        if connect.len() < wanted {
> +            log::debug!(
>                  target: "service",
> -                "Not enough available peers to connect to (available={}, target={target})",
> -                available.len()

Comment #3.

> +                "Not enough available peers to connect to (available={}, wanted={wanted})",

Comment #4.

> +                connect.len()
>              );
>          }
> -        for (id, ka) in available {
> +        for (id, ka) in connect {
>              self.connect(id, ka.addr.clone());
>          }
>     }

Comment #5.

"#;

        let commit = Oid::from_str("a32c4b93e2573fd83b15ac1ad6bf1317dc8fd760").unwrap();
        let path = PathBuf::from_str("main.rs").unwrap();
        let expected = &[
            (ReviewComment {
                location: CodeLocation {
                    commit,
                    path: path.clone(),
                    old: Some(CodeRange::Lines { range: 2559..2565 }),
                    new: Some(CodeRange::Lines { range: 2560..2563 }),
                },
                body: "Comment #1.".to_owned(),
            }),
            (ReviewComment {
                location: CodeLocation {
                    commit,
                    path: path.clone(),
                    old: Some(CodeRange::Lines { range: 2565..2568 }),
                    new: Some(CodeRange::Lines { range: 2563..2567 }),
                },
                body: "Comment #2.".to_owned(),
            }),
            (ReviewComment {
                location: CodeLocation {
                    commit,
                    path: path.clone(),
                    old: Some(CodeRange::Lines { range: 2568..2571 }),
                    new: Some(CodeRange::Lines { range: 2567..2570 }),
                },
                body: "Comment #3.".to_owned(),
            }),
            (ReviewComment {
                location: CodeLocation {
                    commit,
                    path: path.clone(),
                    old: None,
                    new: Some(CodeRange::Lines { range: 2570..2571 }),
                },
                body: "Comment #4.".to_owned(),
            }),
            (ReviewComment {
                location: CodeLocation {
                    commit,
                    path: path.clone(),
                    old: Some(CodeRange::Lines { range: 2571..2577 }),
                    new: Some(CodeRange::Lines { range: 2571..2578 }),
                },
                body: "Comment #5.".to_owned(),
            }),
        ];

        let mut builder = CommentBuilder::new(commit, path.clone());
        builder.add_hunk(
            HunkHeader {
                old_line_no: 2559,
                old_size: 18,
                new_line_no: 2560,
                new_size: 18,
                text: vec![],
            },
            input,
        );
        let actual = builder.comments();

        assert_eq!(actual.len(), expected.len(), "{actual:#?}");

        for (left, right) in actual.iter().zip(expected) {
            assert_eq!(left, right);
        }
    }

    #[test]
    fn test_review_comments_multiline() {
        let input = r#"
> @@ -2559,9 +2560,7 @@ where
>                  // Only consider onion addresses if configured.
>                  AddressType::Onion => self.config.onion.is_some(),
>                  AddressType::Dns | AddressType::Ipv4 | AddressType::Ipv6 => true,
> -            })
> -            .take(wanted)
> -            .collect::<Vec<_>>(); // # -2564

Blah blah blah blah blah blah blah.
Blah blah blah.

Blaah blaah blaah blaah blaah blaah blaah.
blaah blaah blaah.

Blaaah blaaah blaaah.

> +            });
>
> -        if available.len() < target {
> -            log::warn!( # -2567
> +        // Peers we are going to attempt connections to.
> +        let connect = available.take(wanted).collect::<Vec<_>>();

Woof woof.
Woof.
Woof.

Woof.

"#;

        let commit = Oid::from_str("a32c4b93e2573fd83b15ac1ad6bf1317dc8fd760").unwrap();
        let path = PathBuf::from_str("main.rs").unwrap();
        let expected = &[
            (ReviewComment {
                location: CodeLocation {
                    commit,
                    path: path.clone(),
                    old: Some(CodeRange::Lines { range: 2559..2565 }),
                    new: Some(CodeRange::Lines { range: 2560..2563 }),
                },
                body: r#"
Blah blah blah blah blah blah blah.
Blah blah blah.

Blaah blaah blaah blaah blaah blaah blaah.
blaah blaah blaah.

Blaaah blaaah blaaah.
"#
                .trim()
                .to_owned(),
            }),
            (ReviewComment {
                location: CodeLocation {
                    commit,
                    path: path.clone(),
                    old: Some(CodeRange::Lines { range: 2565..2568 }),
                    new: Some(CodeRange::Lines { range: 2563..2567 }),
                },
                body: r#"
Woof woof.
Woof.
Woof.

Woof.
"#
                .trim()
                .to_owned(),
            }),
        ];

        let mut builder = CommentBuilder::new(commit, path.clone());
        builder.add_hunk(
            HunkHeader {
                old_line_no: 2559,
                old_size: 9,
                new_line_no: 2560,
                new_size: 7,
                text: vec![],
            },
            input,
        );
        let actual = builder.comments();

        assert_eq!(actual.len(), expected.len(), "{actual:#?}");

        for (left, right) in actual.iter().zip(expected) {
            assert_eq!(left, right);
        }
    }

    #[test]
    fn test_review_comments_before() {
        let input = r#"
This is a top-level comment.

> @@ -2559,9 +2560,7 @@ where
>                  // Only consider onion addresses if configured.
>                  AddressType::Onion => self.config.onion.is_some(),
>                  AddressType::Dns | AddressType::Ipv4 | AddressType::Ipv6 => true,
> -            })
> -            .take(wanted)
> -            .collect::<Vec<_>>(); // # -2564
> +            });
>
> -        if available.len() < target {
> -            log::warn!( # -2567
> +        // Peers we are going to attempt connections to.
> +        let connect = available.take(wanted).collect::<Vec<_>>();
"#;

        let commit = Oid::from_str("a32c4b93e2573fd83b15ac1ad6bf1317dc8fd760").unwrap();
        let path = PathBuf::from_str("main.rs").unwrap();
        let expected = &[(ReviewComment {
            location: CodeLocation {
                commit,
                path: path.clone(),
                old: Some(CodeRange::Lines { range: 2559..2569 }),
                new: Some(CodeRange::Lines { range: 2560..2568 }),
            },
            body: "This is a top-level comment.".to_owned(),
        })];

        let mut builder = CommentBuilder::new(commit, path.clone());
        builder.add_hunk(
            HunkHeader {
                old_line_no: 2559,
                old_size: 9,
                new_line_no: 2560,
                new_size: 7,
                text: vec![],
            },
            input,
        );
        let actual = builder.comments();

        assert_eq!(actual.len(), expected.len(), "{actual:#?}");

        for (left, right) in actual.iter().zip(expected) {
            assert_eq!(left, right);
        }
    }

    #[test]
    fn test_review_comments_split_hunk() {
        let input = r#"
> @@ -2559,6 +2560,4 @@ where
>                  // Only consider onion addresses if configured.
>                  AddressType::Onion => self.config.onion.is_some(),
>                  AddressType::Dns | AddressType::Ipv4 | AddressType::Ipv6 => true,
> -            })
> -            .take(wanted)

> -            .collect::<Vec<_>>();
> +            });

Comment on a split hunk.
"#;

        let commit = Oid::from_str("a32c4b93e2573fd83b15ac1ad6bf1317dc8fd760").unwrap();
        let path = PathBuf::from_str("main.rs").unwrap();
        let expected = &[(ReviewComment {
            location: CodeLocation {
                commit,
                path: path.clone(),
                old: Some(CodeRange::Lines { range: 2564..2565 }),
                new: Some(CodeRange::Lines { range: 2563..2564 }),
            },
            body: "Comment on a split hunk.".to_owned(),
        })];

        let mut builder = CommentBuilder::new(commit, path.clone());
        builder.add_hunk(
            HunkHeader {
                old_line_no: 2559,
                old_size: 6,
                new_line_no: 2560,
                new_size: 4,
                text: vec![],
            },
            input,
        );
        let actual = builder.comments();

        assert_eq!(actual.len(), expected.len(), "{actual:#?}");

        for (left, right) in actual.iter().zip(expected) {
            assert_eq!(left, right);
        }
    }
}
