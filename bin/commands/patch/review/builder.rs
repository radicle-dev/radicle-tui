//! Review builder.
//!
//! This module enables a user to review a patch by interactively viewing and accepting diff hunks.
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
use std::io;
use std::ops::{Not, Range};
use std::path::PathBuf;

use radicle::cob::patch::{PatchId, Revision};
use radicle::cob::{CodeLocation, CodeRange};
use radicle::git;
use radicle::git::Oid;
use radicle::prelude::*;
use radicle::storage::git::Repository;
use radicle_surf::diff::*;

use radicle_cli::git::unified_diff::{self, FileHeader};
use radicle_cli::git::unified_diff::{Encode, HunkHeader};
use radicle_cli::terminal as term;

use crate::git::HunkDiff;

/// Queue of items (usually hunks) left to review.
#[derive(Clone, Default)]
pub struct Hunks {
    hunks: Vec<(usize, HunkDiff)>,
}

impl Hunks {
    pub fn new(base: Diff) -> Self {
        let base_files = base.into_files();

        let mut queue = Self::default();
        for file in base_files {
            queue.add_file(file);
        }
        queue
    }

    /// Add a file to the queue.
    /// Mostly splits files into individual review items (eg. hunks) to review.
    fn add_file(&mut self, file: FileDiff) {
        let header = FileHeader::from(&file);

        match file {
            FileDiff::Moved(moved) => {
                self.add_item(HunkDiff::Moved { moved });
            }
            FileDiff::Copied(copied) => {
                self.add_item(HunkDiff::Copied {
                    copied: copied.clone(),
                });
            }
            FileDiff::Added(a) => {
                self.add_item(HunkDiff::Added {
                    path: a.path.clone(),
                    header: header.clone(),
                    new: a.new.clone(),
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
                self.add_item(HunkDiff::Deleted {
                    path: d.path.clone(),
                    header: header.clone(),
                    old: d.old.clone(),
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
                    self.add_item(HunkDiff::ModeChanged {
                        path: m.path.clone(),
                        header: header.clone(),
                        old: m.old.clone(),
                        new: m.new.clone(),
                    });
                }
                match m.diff.clone() {
                    DiffContent::Empty => {
                        // Likely a file mode change, which is handled above.
                    }
                    DiffContent::Binary => {
                        self.add_item(HunkDiff::Modified {
                            path: m.path.clone(),
                            header: header.clone(),
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
                        let base_hunks = hunks.clone();

                        for hunk in base_hunks {
                            self.add_item(HunkDiff::Modified {
                                path: m.path.clone(),
                                header: header.clone(),
                                old: m.old.clone(),
                                new: m.new.clone(),
                                hunk: Some(hunk),
                                _stats: Some(stats),
                            });
                        }
                        if let EofNewLine::OldMissing | EofNewLine::NewMissing = eof {
                            self.add_item(HunkDiff::EofChanged {
                                path: m.path.clone(),
                                header: header.clone(),
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

    fn add_item(&mut self, item: HunkDiff) {
        self.hunks.push((self.hunks.len(), item));
    }
}

impl std::ops::Deref for Hunks {
    type Target = Vec<(usize, HunkDiff)>;

    fn deref(&self) -> &Self::Target {
        &self.hunks
    }
}

impl std::ops::DerefMut for Hunks {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.hunks
    }
}

/// Builds a review for a single file.
/// Adjusts line deltas when a hunk is ignored.
pub struct FileReviewBuilder {
    delta: i32,
    header: FileHeader,
}

impl FileReviewBuilder {
    pub fn new(item: &HunkDiff) -> Self {
        Self {
            delta: 0,
            header: item.file_header(),
        }
    }

    pub fn set_item(&mut self, item: &HunkDiff) -> &mut Self {
        let header = item.file_header();
        if self.header != header {
            self.header = header;
            self.delta = 0;
        }
        self
    }

    pub fn item_diff(&mut self, item: &HunkDiff) -> Result<git::raw::Diff, Error> {
        let mut buf = Vec::new();
        let mut writer = unified_diff::Writer::new(&mut buf);
        writer.encode(&self.header)?;

        if let (Some(h), Some(mut header)) = (item.hunk(), item.hunk_header()) {
            header.old_line_no -= self.delta as u32;
            header.new_line_no -= self.delta as u32;

            let h = Hunk {
                header: header.to_unified_string()?.as_bytes().to_owned().into(),
                lines: h.lines.clone(),
                old: h.old.clone(),
                new: h.new.clone(),
            };
            writer.encode(&h)?;
        }
        drop(writer);

        git::raw::Diff::from_buffer(&buf).map_err(Error::from)
    }
}

/// Represents the reviewer's brain, ie. what they have seen or not seen in terms
/// of changes introduced by a patch.
#[derive(Clone)]
pub struct Brain<'a> {
    /// Where the review draft is being stored.
    refname: git::Namespaced<'a>,
    /// The merge base
    base: git::raw::Commit<'a>,
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
        base: git::raw::Commit<'a>,
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
            base,
            head,
            accepted: tree,
        })
    }

    /// Load an existing brain from the repository.
    pub fn load(
        patch: PatchId,
        remote: &NodeId,
        base: git::raw::Commit<'a>,
        repo: &'a git::raw::Repository,
    ) -> Result<Self, git::raw::Error> {
        // TODO: Validate this leads to correct UX for potentially abandoned drafts on
        // past revisions.
        let refname = Self::refname(&patch, remote);
        let head = repo.find_reference(&refname)?.peel_to_commit()?;
        let tree = head.tree()?;

        Ok(Self {
            refname,
            base,
            head,
            accepted: tree,
        })
    }

    pub fn load_or_new<G: Signer>(
        patch: PatchId,
        revision: &Revision,
        repo: &'a git::raw::Repository,
        signer: &'a G,
    ) -> Result<Self, git::raw::Error> {
        let base = repo.find_commit((*revision.base()).into())?;

        let brain = if let Ok(b) = Brain::load(patch, signer.public_key(), base.clone(), repo) {
            log::info!(
                "Loaded existing brain {} for patch {}",
                b.head().id(),
                &patch
            );
            b
        } else {
            Brain::new(patch, signer.public_key(), base, repo)?
        };

        Ok(brain)
    }

    pub fn discard_accepted(
        &mut self,
        repo: &'a git::raw::Repository,
    ) -> Result<(), git::raw::Error> {
        // Reset brain
        let head = self.head.amend(
            Some(&self.refname),
            None,
            None,
            None,
            None,
            Some(&self.base.tree()?),
        )?;
        self.head = repo.find_commit(head)?;
        self.accepted = self.head.tree()?;

        Ok(())
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

pub struct DiffUtil<'a> {
    repo: &'a Repository,
}

impl<'a> DiffUtil<'a> {
    pub fn new(repo: &'a Repository) -> Self {
        Self { repo }
    }

    pub fn all_diffs(&self, revision: &Revision) -> anyhow::Result<Diff> {
        let repo = self.repo.raw();

        let base = repo.find_commit((*revision.base()).into())?.tree()?;
        let revision = {
            let commit = repo.find_commit(revision.head().into())?;
            commit.tree()?
        };

        let mut opts = git::raw::DiffOptions::new();
        opts.patience(true).minimal(true).context_lines(3_u32);

        let base_diff = self.diff(&base, &revision, repo, &mut opts)?;

        Ok(base_diff)
    }

    pub fn rejected_diffs(&self, brain: &Brain<'a>, revision: &Revision) -> anyhow::Result<Diff> {
        let repo = self.repo.raw();
        let revision = {
            let commit = repo.find_commit(revision.head().into())?;
            commit.tree()?
        };

        let mut opts = git::raw::DiffOptions::new();
        opts.patience(true).minimal(true).context_lines(3_u32);

        let rejected = self.diff(brain.accepted(), &revision, repo, &mut opts)?;

        Ok(rejected)
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

/// Builds a patch review interactively, across multiple files.
pub struct ReviewBuilder<'a> {
    /// Stored copy of repository.
    repo: &'a Repository,
}

impl<'a> ReviewBuilder<'a> {
    /// Create a new review builder.
    pub fn new(repo: &'a Repository) -> Self {
        Self { repo }
    }

    pub fn hunks(&self, revision: &Revision) -> anyhow::Result<Hunks> {
        let diff = DiffUtil::new(self.repo).all_diffs(revision)?;
        Ok(Hunks::new(diff))
    }

    // pub fn rejected_hunks(
    //     &self,
    //     brain: &'a Brain<'a>,
    //     revision: &Revision,
    // ) -> anyhow::Result<Hunks> {
    //     let diff = DiffUtil::new(self.repo).rejected_diffs(brain, revision)?;
    //     Ok(Hunks::new(diff))
    // }
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

        let output = term::Editor::comment()
            .extension("diff")
            .initial(input)?
            .edit()?;

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
    use std::str::FromStr;

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
