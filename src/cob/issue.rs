use anyhow::Result;
use radicle::cob::issue::{Issue, IssueId, Issues, State};
use radicle::cob::Label;
use radicle::prelude::{Did, Signer};
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
    labels: &[Label],
    assignees: &[Did],
) -> Result<IssueId> {
    let mut issues = Issues::open(repository)?;
    let issue = issues.create(title, description.trim(), labels, assignees, signer)?;

    Ok(*issue.id())
}

#[allow(clippy::too_many_arguments)]
pub fn edit<G: Signer>(
    repository: &Repository,
    signer: &G,
    id: IssueId,
    title: String,
    description: String,
    labels: &[Label],
    assignees: &[Did],
    state: State,
) -> Result<IssueId> {
    let mut issues = Issues::open(repository)?;
    let mut issue = issues.get_mut(&id)?;
    
    issue.edit(title, signer)?;
    issue.edit_description(description, signer)?;
    issue.assign(assignees.to_owned(), signer)?;
    issue.label(labels.to_owned(), signer)?;
    issue.lifecycle(state, signer)?;

    Ok(*issue.id())
}
