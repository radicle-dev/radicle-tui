use std::fmt;
use std::fmt::Debug;
use std::path::Path;
use std::{fs, path::PathBuf};

use ratatui::text::Line;

use radicle_surf::diff::{Copied, DiffFile, EofNewLine, FileStats, Hunk, Modification, Moved};

use radicle::git;
use radicle::git::Oid;

use radicle_cli::git::unified_diff::FileHeader;
use radicle_cli::terminal;
use radicle_cli::terminal::highlight::Highlighter;
use serde::{Deserialize, Serialize};

pub type FilePaths<'a> = (Option<(&'a Path, Oid)>, Option<(&'a Path, Oid)>);

/// Get the diff stats between two commits.
/// Should match the default output of `git diff <old> <new> --stat` exactly.
pub fn diff_stats(
    repo: &git::raw::Repository,
    old: &Oid,
    new: &Oid,
) -> Result<git::raw::DiffStats, git::raw::Error> {
    let old = repo.find_commit(**old)?;
    let new = repo.find_commit(**new)?;
    let old_tree = old.tree()?;
    let new_tree = new.tree()?;
    let mut diff = repo.diff_tree_to_tree(Some(&old_tree), Some(&new_tree), None)?;
    let mut find_opts = git::raw::DiffFindOptions::new();

    diff.find_similar(Some(&mut find_opts))?;
    diff.stats()
}

/// A repository of Git blobs.
pub trait Repo {
    /// Lookup a blob from the repo.
    fn blob(&self, oid: git::Oid) -> Result<Blob, git::raw::Error>;
    /// Lookup a file in the workdir.
    fn file(&self, path: &Path) -> Option<Blob>;
}

impl Repo for git::raw::Repository {
    fn blob(&self, oid: git::Oid) -> Result<Blob, git::raw::Error> {
        let blob = self.find_blob(*oid)?;

        if blob.is_binary() {
            Ok(Blob::Binary)
        } else {
            let content = blob.content();

            if content.is_empty() {
                Ok(Blob::Empty)
            } else {
                Ok(Blob::Plain(blob.content().to_vec()))
            }
        }
    }

    fn file(&self, path: &Path) -> Option<Blob> {
        self.workdir()
            .and_then(|dir| fs::read(dir.join(path)).ok())
            .map(|content| {
                // A file is considered binary if there is a zero byte in the first 8 kilobytes
                // of the file. This is the same heuristic Git uses.
                let binary = content.iter().take(8192).any(|b| *b == 0);
                if binary {
                    Blob::Binary
                } else {
                    Blob::Plain(content)
                }
            })
    }
}

/// Blob returned by the [`Repo`] trait.
#[derive(PartialEq, Eq, Debug)]
pub enum Blob {
    Binary,
    Empty,
    Plain(Vec<u8>),
}

/// Blobs passed down to the hunk renderer.
#[derive(Clone, Debug)]
pub struct Blobs<T> {
    pub old: Option<T>,
    pub new: Option<T>,
}

impl<T> Blobs<T> {
    pub fn new(old: Option<T>, new: Option<T>) -> Self {
        Self { old, new }
    }
}

impl<'a> Blobs<(PathBuf, Blob)> {
    pub fn highlight(self, mut hi: Highlighter) -> Blobs<Vec<Line<'a>>> {
        let mut blobs = Blobs::default();
        if let Some((path, Blob::Plain(content))) = &self.old {
            blobs.old = hi
                .highlight(path, content)
                .map(|hi| {
                    hi.into_iter()
                        .map(|line| Line::raw(line.to_string()))
                        .collect::<Vec<_>>()
                })
                .ok();
        }
        if let Some((path, Blob::Plain(content))) = &self.new {
            blobs.new = hi
                .highlight(path, content)
                .map(|hi| {
                    hi.into_iter()
                        .map(|line| Line::raw(line.to_string()))
                        .collect::<Vec<_>>()
                })
                .ok();
        }
        blobs
    }

    pub fn _raw(self) -> Blobs<Vec<Line<'a>>> {
        let mut blobs = Blobs::default();
        if let Some((_, Blob::Plain(content))) = &self.old {
            blobs.old = std::str::from_utf8(content)
                .map(|lines| {
                    lines
                        .lines()
                        .map(terminal::Line::new)
                        .map(|line| Line::raw(line.to_string()))
                        .collect::<Vec<_>>()
                })
                .ok();
        }
        if let Some((_, Blob::Plain(content))) = &self.new {
            blobs.new = std::str::from_utf8(content)
                .map(|lines| {
                    lines
                        .lines()
                        .map(terminal::Line::new)
                        .map(|line| Line::raw(line.to_string()))
                        .collect::<Vec<_>>()
                })
                .ok();
        }
        blobs
    }

    pub fn from_paths<R: Repo>(
        old: Option<(&Path, Oid)>,
        new: Option<(&Path, Oid)>,
        repo: &R,
    ) -> Blobs<(PathBuf, Blob)> {
        Blobs::new(
            old.and_then(|(path, oid)| {
                repo.blob(oid)
                    .ok()
                    .or_else(|| repo.file(path))
                    .map(|blob| (path.to_path_buf(), blob))
            }),
            new.and_then(|(path, oid)| {
                repo.blob(oid)
                    .ok()
                    .or_else(|| repo.file(path))
                    .map(|blob| (path.to_path_buf(), blob))
            }),
        )
    }
}

impl<T> Default for Blobs<T> {
    fn default() -> Self {
        Self {
            old: None,
            new: None,
        }
    }
}

pub enum DiffStats {
    Hunk(HunkStats),
    File(FileStats),
}

#[derive(Default)]
pub struct HunkStats {
    added: usize,
    deleted: usize,
}

impl HunkStats {
    pub fn added(&self) -> usize {
        self.added
    }
    pub fn deleted(&self) -> usize {
        self.deleted
    }
}

impl From<&Hunk<Modification>> for HunkStats {
    fn from(hunk: &Hunk<Modification>) -> Self {
        let mut added = 0_usize;
        let mut deleted = 0_usize;

        for modification in &hunk.lines {
            match modification {
                Modification::Addition(_) => added += 1,
                Modification::Deletion(_) => deleted += 1,
                _ => {}
            }
        }

        Self { added, deleted }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub enum HunkState {
    #[default]
    Unknown,
    Rejected,
    Accepted,
}

/// A single review item. Can be a hunk or eg. a file move.
/// Files are usually split into multiple review items.
#[derive(Clone, PartialEq)]
pub enum HunkDiff {
    Added {
        path: PathBuf,
        header: FileHeader,
        new: DiffFile,
        hunk: Option<Hunk<Modification>>,
        _stats: Option<FileStats>,
    },
    Deleted {
        path: PathBuf,
        header: FileHeader,
        old: DiffFile,
        hunk: Option<Hunk<Modification>>,
        _stats: Option<FileStats>,
    },
    Modified {
        path: PathBuf,
        header: FileHeader,
        old: DiffFile,
        new: DiffFile,
        hunk: Option<Hunk<Modification>>,
        _stats: Option<FileStats>,
    },
    Moved {
        moved: Moved,
    },
    Copied {
        copied: Copied,
    },
    EofChanged {
        path: PathBuf,
        header: FileHeader,
        old: DiffFile,
        new: DiffFile,
        _eof: EofNewLine,
    },
    ModeChanged {
        path: PathBuf,
        header: FileHeader,
        old: DiffFile,
        new: DiffFile,
    },
}

impl HunkDiff {
    pub fn hunk(&self) -> Option<&Hunk<Modification>> {
        match self {
            Self::Added { hunk, .. } => hunk.as_ref(),
            Self::Deleted { hunk, .. } => hunk.as_ref(),
            Self::Modified { hunk, .. } => hunk.as_ref(),
            _ => None,
        }
    }

    pub fn path(&self) -> &PathBuf {
        match self {
            HunkDiff::Added { path, .. } => path,
            HunkDiff::Modified { path, .. } => path,
            HunkDiff::Deleted { path, .. } => path,
            HunkDiff::Copied { copied } => &copied.new_path,
            HunkDiff::Moved { moved } => &moved.new_path,
            HunkDiff::ModeChanged { path, .. } => path,
            HunkDiff::EofChanged { path, .. } => path,
        }
    }

    pub fn paths(&self) -> FilePaths<'_> {
        match self {
            Self::Added { path, new, .. } => (None, Some((path, new.oid))),
            Self::Deleted { path, old, .. } => (Some((path, old.oid)), None),
            Self::Moved { moved } => (
                Some((&moved.old_path, moved.old.oid)),
                Some((&moved.new_path, moved.new.oid)),
            ),
            Self::Copied { copied } => (
                Some((&copied.old_path, copied.old.oid)),
                Some((&copied.new_path, copied.new.oid)),
            ),
            Self::Modified { path, old, new, .. } => (Some((path, old.oid)), Some((path, new.oid))),
            Self::EofChanged { path, old, new, .. } => {
                (Some((path, old.oid)), Some((path, new.oid)))
            }
            Self::ModeChanged { path, old, new, .. } => {
                (Some((path, old.oid)), Some((path, new.oid)))
            }
        }
    }

    pub fn blobs<R: Repo>(&self, repo: &R) -> Blobs<(PathBuf, Blob)> {
        let (old, new) = self.paths();
        Blobs::from_paths(old, new, repo)
    }
}

impl Debug for HunkDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (name, path, hunk) = match self {
            Self::Added { path, hunk, .. } => ("Added", path, hunk),
            Self::Deleted { path, hunk, .. } => ("Deleted", path, hunk),
            Self::Moved { moved } => ("Moved", &moved.new_path, &None),
            Self::Copied { copied } => ("Copied", &copied.new_path, &None),
            Self::Modified { path, hunk, .. } => ("Modified", path, hunk),
            Self::EofChanged { path, .. } => ("EofChanged", path, &None),
            Self::ModeChanged { path, .. } => ("ModeChanged", path, &None),
        };

        match hunk {
            Some(hunk) => f
                .debug_struct(name)
                .field("path", path)
                .field("hunk", &(hunk.old.clone(), hunk.new.clone()))
                .finish(),
            _ => f.debug_struct(name).field("path", path).finish(),
        }
    }
}
