use std::fs;
use std::path::Path;

use radicle::git;
use radicle::git::Oid;

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

/// Blob returned by the [`Repo`] trait.
#[derive(PartialEq, Eq, Debug)]
pub enum Blob {
    Binary,
    Empty,
    Plain(Vec<u8>),
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
