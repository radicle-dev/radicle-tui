use std::str::FromStr;

use anyhow::Result;

use radicle::cob::Label;
use radicle::prelude::Did;

use std::path::{Path, PathBuf};

use radicle::git::Oid;

use radicle_surf::diff::*;

use radicle_cli::git::unified_diff::FileHeader;
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

/// A single review item. Can be a hunk or eg. a file move.
/// Files are usually split into multiple review items.
#[derive(Clone, Debug)]
pub enum ReviewItem {
    FileAdded {
        path: PathBuf,
        header: FileHeader,
        new: DiffFile,
        hunk: Option<Hunk<Modification>>,
        stats: Option<FileStats>,
    },
    FileDeleted {
        path: PathBuf,
        header: FileHeader,
        old: DiffFile,
        hunk: Option<Hunk<Modification>>,
        stats: Option<FileStats>,
    },
    FileModified {
        path: PathBuf,
        header: FileHeader,
        old: DiffFile,
        new: DiffFile,
        hunk: Option<Hunk<Modification>>,
        stats: Option<FileStats>,
    },
    FileMoved {
        moved: Moved,
    },
    FileCopied {
        copied: Copied,
    },
    FileEofChanged {
        path: PathBuf,
        header: FileHeader,
        old: DiffFile,
        new: DiffFile,
        eof: EofNewLine,
    },
    FileModeChanged {
        path: PathBuf,
        header: FileHeader,
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

    pub fn file_header(&self) -> FileHeader {
        match self {
            Self::FileAdded { header, .. } => header.clone(),
            Self::FileDeleted { header, .. } => header.clone(),
            Self::FileMoved { moved } => FileHeader::Moved {
                old_path: moved.old_path.clone(),
                new_path: moved.new_path.clone(),
            },
            Self::FileCopied { copied } => FileHeader::Copied {
                old_path: copied.old_path.clone(),
                new_path: copied.new_path.clone(),
            },
            Self::FileModified { header, .. } => header.clone(),
            Self::FileEofChanged { header, .. } => header.clone(),
            Self::FileModeChanged { header, .. } => header.clone(),
        }
    }

    pub fn blobs<R: Repo>(&self, repo: &R) -> Blobs<(PathBuf, Blob)> {
        let (old, new) = self.paths();
        Blobs::from_paths(old, new, repo)
    }
}
