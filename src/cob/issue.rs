use std::fmt::Display;

use anyhow::Result;
use radicle::cob::issue::{Issue, IssueId, Issues};
use radicle::cob::Label;
use radicle::prelude::{Did, Signer};
use radicle::storage::git::Repository;
use radicle::{issue, Profile};

use super::format;

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub enum State {
    #[default]
    Open,
    Closed,
}

impl Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = match self {
            State::Open => "open",
            State::Closed => "closed",
        };
        f.write_str(state)
    }
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct Filter {
    state: Option<State>,
    authored: bool,
    authors: Vec<Did>,
}

impl Filter {
    pub fn with_state(mut self, state: Option<State>) -> Self {
        self.state = state;
        self
    }

    pub fn with_authored(mut self, authored: bool) -> Self {
        self.authored = authored;
        self
    }

    pub fn with_author(mut self, author: Did) -> Self {
        self.authors.push(author);
        self
    }

    pub fn matches(&self, profile: &Profile, issue: &Issue) -> bool {
        let matches_state = match self.state {
            Some(State::Open) => matches!(issue.state(), issue::State::Open),
            Some(State::Closed) => matches!(issue.state(), issue::State::Closed { .. }),
            None => true,
        };

        let matches_authored = self
            .authored
            .then(|| *issue.author().id() == profile.did())
            .unwrap_or(true);

        let matches_authors = (!self.authors.is_empty())
            .then(|| {
                self.authors
                    .iter()
                    .any(|other| *issue.author().id() == *other)
            })
            .unwrap_or(true);

        matches_state && matches_authored && matches_authors
    }
}

impl ToString for Filter {
    fn to_string(&self) -> String {
        let mut filter = String::new();
        filter.push(' ');

        if let Some(state) = &self.state {
            filter.push_str(&format!("is:{}", state));
            filter.push(' ');
        }
        if self.authored {
            filter.push_str("is:authored");
            filter.push(' ');
        }
        if !self.authors.is_empty() {
            filter.push_str("authors:");
            filter.push('[');

            let mut authors = self.authors.iter().peekable();
            while let Some(author) = authors.next() {
                filter.push_str(&format::did(author));

                if authors.peek().is_some() {
                    filter.push(',');
                }
            }
            filter.push(']');
        }

        filter
    }
}

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
    let issue = issues.create(title, description.trim(), labels, assignees, [], signer)?;

    Ok(*issue.id())
}
