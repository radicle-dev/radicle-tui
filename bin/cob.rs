use std::str::FromStr;

use anyhow::Result;

use radicle::cob::Label;
use radicle::prelude::Did;

use std::path::{Path, PathBuf};

use radicle::git::Oid;

use radicle_surf::diff::*;

use radicle_cli::git::unified_diff::HunkHeader;

use crate::git::Blob;
use crate::git::Repo;
use crate::ui::items::Blobs;

pub mod inbox;
pub mod issue;
pub mod patch;

pub type IndexedReviewItem = (usize, crate::cob::ReviewItem);

#[allow(dead_code)]
pub fn parse_labels(input: String) -> Result<Vec<Label>> {
    let mut labels = vec![];
    if !input.is_empty() {
        for name in input.split(',') {
            match Label::new(name.trim()) {
                Ok(label) => labels.push(label),
                Err(err) => return Err(anyhow::anyhow!(err).context("Can't parse labels.")),
            }
        }
    }

    Ok(labels)
}

#[allow(dead_code)]
pub fn parse_assignees(input: String) -> Result<Vec<Did>> {
    let mut assignees = vec![];
    if !input.is_empty() {
        for did in input.split(',') {
            match Did::from_str(&format!("did:key:{}", did)) {
                Ok(did) => assignees.push(did),
                Err(err) => return Err(anyhow::anyhow!(err).context("Can't parse assignees.")),
            }
        }
    }

    Ok(assignees)
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

/// A single review item. Can be a hunk or eg. a file move.
/// Files are usually split into multiple review items.
#[derive(Clone, Debug)]
pub enum ReviewItem {
    FileAdded {
        path: PathBuf,
        new: DiffFile,
        hunk: Option<Hunk<Modification>>,
        _stats: Option<FileStats>,
    },
    FileDeleted {
        path: PathBuf,
        old: DiffFile,
        hunk: Option<Hunk<Modification>>,
        _stats: Option<FileStats>,
    },
    FileModified {
        path: PathBuf,
        old: DiffFile,
        new: DiffFile,
        hunk: Option<Hunk<Modification>>,
        _stats: Option<FileStats>,
    },
    FileMoved {
        moved: Moved,
    },
    FileCopied {
        copied: Copied,
    },
    FileEofChanged {
        path: PathBuf,
        old: DiffFile,
        new: DiffFile,
        _eof: EofNewLine,
    },
    FileModeChanged {
        path: PathBuf,
        old: DiffFile,
        new: DiffFile,
    },
}

impl ReviewItem {
    pub fn hunk(&self) -> Option<&Hunk<Modification>> {
        match self {
            Self::FileAdded { hunk, .. } => hunk.as_ref(),
            Self::FileDeleted { hunk, .. } => hunk.as_ref(),
            Self::FileModified { hunk, .. } => hunk.as_ref(),
            _ => None,
        }
    }

    pub fn hunk_header(&self) -> Option<HunkHeader> {
        self.hunk().and_then(|h| HunkHeader::try_from(h).ok())
    }

    pub fn paths(&self) -> (Option<(&Path, Oid)>, Option<(&Path, Oid)>) {
        match self {
            Self::FileAdded { path, new, .. } => (None, Some((path, new.oid))),
            Self::FileDeleted { path, old, .. } => (Some((path, old.oid)), None),
            Self::FileMoved { moved } => (
                Some((&moved.old_path, moved.old.oid)),
                Some((&moved.new_path, moved.new.oid)),
            ),
            Self::FileCopied { copied } => (
                Some((&copied.old_path, copied.old.oid)),
                Some((&copied.new_path, copied.new.oid)),
            ),
            Self::FileModified { path, old, new, .. } => {
                (Some((path, old.oid)), Some((path, new.oid)))
            }
            Self::FileEofChanged { path, old, new, .. } => {
                (Some((path, old.oid)), Some((path, new.oid)))
            }
            Self::FileModeChanged { path, old, new, .. } => {
                (Some((path, old.oid)), Some((path, new.oid)))
            }
        }
    }

    pub fn blobs<R: Repo>(&self, repo: &R) -> Blobs<(PathBuf, Blob)> {
        let (old, new) = self.paths();
        Blobs::from_paths(old, new, repo)
    }
}
