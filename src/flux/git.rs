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
