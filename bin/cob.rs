use std::str::FromStr;

use anyhow::Result;

use radicle::cob::Label;
use radicle::prelude::Did;
use radicle_cli::git::unified_diff::FileHeader;

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

pub type IndexedHunkItem = (usize, crate::cob::HunkItem, HunkState);
pub type FilePaths<'a> = (Option<(&'a Path, Oid)>, Option<(&'a Path, Oid)>);

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

#[derive(Clone, Default, Debug, PartialEq)]
pub enum HunkState {
    #[default]
    Rejected,
    Accepted,
}

/// A single review item. Can be a hunk or eg. a file move.
/// Files are usually split into multiple review items.
#[derive(Clone, Debug)]
pub enum HunkItem {
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

impl HunkItem {
    pub fn hunk(&self) -> Option<&Hunk<Modification>> {
        match self {
            Self::Added { hunk, .. } => hunk.as_ref(),
            Self::Deleted { hunk, .. } => hunk.as_ref(),
            Self::Modified { hunk, .. } => hunk.as_ref(),
            _ => None,
        }
    }

    pub fn file_header(&self) -> FileHeader {
        match self {
            Self::Added { header, .. } => header.clone(),
            Self::Deleted { header, .. } => header.clone(),
            Self::Moved { moved } => FileHeader::Moved {
                old_path: moved.old_path.clone(),
                new_path: moved.new_path.clone(),
            },
            Self::Copied { copied } => FileHeader::Copied {
                old_path: copied.old_path.clone(),
                new_path: copied.new_path.clone(),
            },
            Self::Modified { header, .. } => header.clone(),
            Self::EofChanged { header, .. } => header.clone(),
            Self::ModeChanged { header, .. } => header.clone(),
        }
    }

    pub fn hunk_header(&self) -> Option<HunkHeader> {
        self.hunk().and_then(|h| HunkHeader::try_from(h).ok())
    }

    pub fn paths(&self) -> FilePaths {
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
