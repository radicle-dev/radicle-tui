use anyhow::Result;
use radicle::cob::issue::{Issue, IssueId, Issues};
use radicle::cob::{ActorId, Tag};
use radicle::prelude::Signer;
use radicle::storage::git::Repository;

pub fn all(repository: &Repository) -> Result<Vec<(IssueId, Issue)>> {
    let patches = Issues::open(repository)?
        .all()
        .map(|iter| iter.flatten().collect::<Vec<_>>())?;

    Ok(patches
        .into_iter()
        .map(|(id, issue)| (id, issue))
        .collect::<Vec<_>>())
}

pub fn find(repository: &Repository, id: &IssueId) -> Result<Option<Issue>> {
    let issues = Issues::open(repository)?;
    Ok(issues.get(id)?)
}

pub fn create<G: Signer>(
    repository: &Repository,
    signer: &G,
    title: String,
    description: String,
    tags: &[Tag],
    assignees: &[ActorId],
) -> Result<IssueId> {
    let mut issues = Issues::open(repository)?;
    let issue = issues.create(title, description.trim(), tags, assignees, signer)?;

    Ok(*issue.id())
}
