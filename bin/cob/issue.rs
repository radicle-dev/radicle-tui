use anyhow::Result;

use radicle::cob::issue::{Issue, IssueId};
use radicle::issue::cache::Issues;
use radicle::storage::git::Repository;
use radicle::Profile;

pub fn all(profile: &Profile, repository: &Repository) -> Result<Vec<(IssueId, Issue)>> {
    let cache = profile.issues(repository)?;
    let issues = cache.list()?;

    Ok(issues.flatten().collect())
}
