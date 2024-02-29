use std::fmt::Display;

use anyhow::Result;
use radicle::cob::issue::{Issue, IssueId};
use radicle::cob::Label;
use radicle::issue::cache::Issues;
use radicle::issue::CloseReason;
use radicle::prelude::{Did, Signer};
use radicle::storage::git::Repository;
use radicle::{issue, Profile};

use super::format;

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub enum State {
    #[default]
    Open,
    Solved,
    Closed,
}

impl Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = match self {
            State::Open => "open",
            State::Solved => "solved",
            State::Closed => "closed",
        };
        f.write_str(state)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Filter {
    state: Option<State>,
    assigned: bool,
    assignees: Vec<Did>,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            state: Some(State::default()),
            assigned: false,
            assignees: vec![],
        }
    }
}

impl Filter {
    pub fn with_state(mut self, state: Option<State>) -> Self {
        self.state = state;
        self
    }

    pub fn with_assgined(mut self, assigned: bool) -> Self {
        self.assigned = assigned;
        self
    }

    pub fn with_assginee(mut self, assignee: Did) -> Self {
        self.assignees.push(assignee);
        self
    }

    pub fn matches(&self, profile: &Profile, issue: &Issue) -> bool {
        let matches_state = match self.state {
            Some(State::Open) => matches!(issue.state(), issue::State::Open),
            Some(State::Solved) => matches!(
                issue.state(),
                issue::State::Closed {
                    reason: CloseReason::Solved
                }
            ),
            Some(State::Closed) => matches!(issue.state(), issue::State::Closed { .. }),
            None => true,
        };

        let matches_assgined = self
            .assigned
            .then(|| {
                issue
                    .assignees()
                    .collect::<Vec<_>>()
                    .contains(&&profile.did())
            })
            .unwrap_or(true);

        let matches_assignees = (!self.assignees.is_empty())
            .then(|| {
                self.assignees
                    .iter()
                    .any(|other| issue.assignees().collect::<Vec<_>>().contains(&other))
            })
            .unwrap_or(true);

        matches_state && matches_assgined && matches_assignees
    }
}

impl ToString for Filter {
    fn to_string(&self) -> String {
        let mut filter = String::new();

        if let Some(state) = &self.state {
            filter.push_str(&format!("is:{}", state));
            filter.push(' ');
        }
        if self.assigned {
            filter.push_str("is:assigned");
            filter.push(' ');
        }
        if !self.assignees.is_empty() {
            filter.push_str("assignees:");
            filter.push('[');

            let mut assignees = self.assignees.iter().peekable();
            while let Some(assignee) = assignees.next() {
                filter.push_str(&format::did(assignee));

                if assignees.peek().is_some() {
                    filter.push(',');
                }
            }
            filter.push(']');
        }

        filter
    }
}

pub fn all(profile: &Profile, repository: &Repository) -> Result<Vec<(IssueId, Issue)>> {
    let cache = profile.issues(repository)?;
    let issues = cache.list()?;
    
    let mut all = vec![];
    for issue in issues {
        if let Ok((id, issue)) = issue {
            all.push((id, issue))
        }
    }

    Ok(all)
}

pub fn find(profile: &Profile, repository: &Repository, id: &IssueId) -> Result<Option<Issue>> {
    let cache = profile.issues(repository)?;
    Ok(cache.get(id)?)
}

pub fn create<G: Signer>(
    profile: &Profile,
    repository: &Repository,
    signer: &G,
    title: String,
    description: String,
    labels: &[Label],
    assignees: &[Did],
) -> Result<IssueId> {
    let mut issues = profile.issues_mut(repository)?;
    let issue = issues.create(title, description.trim(), labels, assignees, [], signer)?;

    Ok(*issue.id())
}
