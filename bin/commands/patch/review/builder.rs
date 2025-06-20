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
use std::fmt::Write as _;
use std::io;
use std::path::PathBuf;

use radicle::cob::patch::Revision;
use radicle::cob::{DiffLocation, HunkIndex};
use radicle::git;
use radicle::git::Oid;
use radicle::prelude::*;
use radicle::storage::git::Repository;
use radicle_surf::diff::*;

use radicle_cli::git::unified_diff::Encode;
use radicle_cli::git::unified_diff::{self, FileHeader};
use radicle_cli::terminal as term;

use crate::git::{Hunk, HunkDiff};

/// Queue of items (usually hunks) left to review.
#[derive(Clone, Default, Debug)]
pub struct Hunks {
    hunks: Vec<HunkDiff>,
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
                        hs.len()
                            .checked_sub(1)
                            .and_then(|i| hs.pop().map(|h| Hunk::new(i, h)))
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
                        hs.len()
                            .checked_sub(1)
                            .and_then(|i| hs.pop().map(|h| Hunk::new(i, h)))
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

                        for (i, hunk) in base_hunks.iter().enumerate() {
                            self.add_item(HunkDiff::Modified {
                                path: m.path.clone(),
                                header: header.clone(),
                                old: m.old.clone(),
                                new: m.new.clone(),
                                hunk: Some(Hunk::new(i, hunk.clone())),
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
        self.hunks.push(item);
    }
}

impl std::ops::Deref for Hunks {
    type Target = Vec<HunkDiff>;

    fn deref(&self) -> &Self::Target {
        &self.hunks
    }
}

impl std::ops::DerefMut for Hunks {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.hunks
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
}

#[derive(Debug, PartialEq, Eq)]
pub struct ReviewComment {
    pub location: DiffLocation,
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
    #[error(transparent)]
    Editor(#[from] term::editor::Error),
}

#[derive(Debug)]
pub struct CommentBuilder {
    base: Oid,
    commit: Oid,
    path: PathBuf,
    comments: Vec<ReviewComment>,
}

impl CommentBuilder {
    pub fn new(base: Oid, commit: Oid, path: PathBuf) -> Self {
        Self {
            base,
            commit,
            path,
            comments: Vec::new(),
        }
    }

    pub fn edit(mut self, hunk: &Hunk) -> Result<Vec<ReviewComment>, Error> {
        let mut input = String::new();
        for line in hunk.inner().to_unified_string()?.lines() {
            writeln!(&mut input, "> {line}")?;
        }
        let output = term::Editor::comment()
            .extension("diff")
            .initial(&input)
            .edit()?;

        if let Some(output) = output {
            self.add_hunk(hunk, &output);
        }
        Ok(self.comments())
    }

    fn add_hunk(&mut self, hunk: &Hunk, input: &str) -> &mut Self {
        let lines = input
            .trim()
            .lines()
            .map(|l| l.trim())
            // Skip the hunk header
            .filter(|l| !l.starts_with("> @@"));
        let mut comment = String::new();
        // Keeps track of the line index within the hunk itself
        let mut line_ix = 0_usize;
        // Keeps track of whether the first comment is at the top-level
        let mut top_level = true;

        for line in lines {
            if line.starts_with('>') {
                if !comment.is_empty() {
                    if top_level {
                        // Top-level comment
                        self.add_comment(hunk.as_index(None), &comment);
                    } else {
                        self.add_comment(
                            hunk.as_index(Some(line_ix.saturating_sub(1)..line_ix)),
                            &comment,
                        );
                    }
                    comment.clear();
                }
                line_ix += 1;
                // Can no longer be a top-level comment
                top_level = false;
            } else {
                comment.push_str(line);
                comment.push('\n');
            }
        }
        if !comment.is_empty() {
            self.add_comment(
                hunk.as_index(Some(line_ix.saturating_sub(1)..line_ix)),
                &comment,
            );
        }
        self
    }

    fn add_comment(&mut self, selection: Option<HunkIndex>, comment: &str) {
        // Empty lines between quoted text can generate empty comments
        // that should be filtered out.
        if comment.trim().is_empty() {
            return;
        }

        self.comments.push(ReviewComment {
            location: DiffLocation {
                base: self.base,
                head: self.commit,
                path: self.path.clone(),
                selection,
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
    use radicle::cob::CodeRange;
    use radicle_surf::diff;
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

        let base = git::raw::Oid::zero().into();
        let commit = Oid::from_str("a32c4b93e2573fd83b15ac1ad6bf1317dc8fd760").unwrap();
        let path = PathBuf::from_str("main.rs").unwrap();
        let expected = &[
            (ReviewComment {
                location: DiffLocation::hunk_level(
                    git::raw::Oid::zero().into(),
                    commit,
                    path.clone(),
                    HunkIndex::new(0, CodeRange::lines(5..6)),
                ),
                body: "Comment #1.".to_owned(),
            }),
            (ReviewComment {
                location: DiffLocation::hunk_level(
                    git::raw::Oid::zero().into(),
                    commit,
                    path.clone(),
                    HunkIndex::new(0, CodeRange::lines(11..12)),
                ),
                body: "Comment #2.".to_owned(),
            }),
            (ReviewComment {
                location: DiffLocation::hunk_level(
                    git::raw::Oid::zero().into(),
                    commit,
                    path.clone(),
                    HunkIndex::new(0, CodeRange::lines(16..17)),
                ),
                body: "Comment #3.".to_owned(),
            }),
            (ReviewComment {
                location: DiffLocation::hunk_level(
                    git::raw::Oid::zero().into(),
                    commit,
                    path.clone(),
                    HunkIndex::new(0, CodeRange::lines(17..18)),
                ),
                body: "Comment #4.".to_owned(),
            }),
            (ReviewComment {
                location: DiffLocation::hunk_level(
                    git::raw::Oid::zero().into(),
                    commit,
                    path.clone(),
                    HunkIndex::new(0, CodeRange::lines(25..26)),
                ),
                body: "Comment #5.".to_owned(),
            }),
        ];

        let mut builder = CommentBuilder::new(base, commit, path.clone());
        builder.add_hunk(
            &Hunk::new(
                0,
                diff::Hunk {
                    header: diff::Line::from(vec![]),
                    lines: std::iter::repeat(diff::Modification::addition(
                        diff::Line::from(vec![]),
                        1,
                    ))
                    .take(26)
                    .collect(),
                    old: 2559..2578,
                    new: 2560..2579,
                },
            ),
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

        let base = git::raw::Oid::zero().into();
        let commit = Oid::from_str("a32c4b93e2573fd83b15ac1ad6bf1317dc8fd760").unwrap();
        let path = PathBuf::from_str("main.rs").unwrap();
        let expected = &[
            (ReviewComment {
                location: DiffLocation::hunk_level(
                    git::raw::Oid::zero().into(),
                    commit,
                    path.clone(),
                    HunkIndex::new(0, CodeRange::lines(5..6)),
                ),
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
                location: DiffLocation::hunk_level(
                    git::raw::Oid::zero().into(),
                    commit,
                    path.clone(),
                    HunkIndex::new(0, CodeRange::lines(11..12)),
                ),
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

        let mut builder = CommentBuilder::new(base, commit, path.clone());
        builder.add_hunk(
            &Hunk::new(
                0,
                diff::Hunk {
                    header: diff::Line::from(vec![]),
                    lines: std::iter::repeat(diff::Modification::addition(
                        diff::Line::from(vec![]),
                        1,
                    ))
                    .take(12)
                    .collect(),
                    old: 2559..2569,
                    new: 2560..2568,
                },
            ),
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

        let base = git::raw::Oid::zero().into();
        let commit = Oid::from_str("a32c4b93e2573fd83b15ac1ad6bf1317dc8fd760").unwrap();
        let path = PathBuf::from_str("main.rs").unwrap();
        let expected = &[(ReviewComment {
            location: DiffLocation::file_level(git::raw::Oid::zero().into(), commit, path.clone()),
            body: "This is a top-level comment.".to_owned(),
        })];

        let mut builder = CommentBuilder::new(base, commit, path.clone());
        builder.add_hunk(
            &Hunk::new(
                0,
                diff::Hunk {
                    header: diff::Line::from(vec![]),
                    lines: std::iter::repeat(diff::Modification::addition(
                        diff::Line::from(vec![]),
                        1,
                    ))
                    .take(12)
                    .collect(),
                    old: 2559..2569,
                    new: 2560..2568,
                },
            ),
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

        let base = git::raw::Oid::zero().into();
        let commit = Oid::from_str("a32c4b93e2573fd83b15ac1ad6bf1317dc8fd760").unwrap();
        let path = PathBuf::from_str("main.rs").unwrap();
        let expected = &[(ReviewComment {
            location: DiffLocation::hunk_level(
                git::raw::Oid::zero().into(),
                commit,
                path.clone(),
                HunkIndex::new(0, CodeRange::lines(6..7)),
            ),
            body: "Comment on a split hunk.".to_owned(),
        })];

        let mut builder = CommentBuilder::new(base, commit, path.clone());
        builder.add_hunk(
            &Hunk::new(
                0,
                diff::Hunk {
                    header: diff::Line::from(vec![]),
                    lines: vec![],
                    old: 2559..2566,
                    new: 2560..2565,
                },
            ),
            input,
        );
        let actual = builder.comments();

        assert_eq!(actual.len(), expected.len(), "{actual:#?}");

        for (left, right) in actual.iter().zip(expected) {
            assert_eq!(left, right);
        }
    }
}
