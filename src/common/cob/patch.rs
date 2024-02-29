use std::fmt::Display;

use anyhow::Result;

use radicle::cob::patch::{Patch, PatchId};
use radicle::identity::Did;
use radicle::patch::cache::Patches;
use radicle::storage::git::Repository;
use radicle::{patch, Profile};

use super::format;

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub enum State {
    Draft,
    #[default]
    Open,
    Merged,
    Archived,
}

impl Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = match self {
            State::Draft => "draft",
            State::Open => "open",
            State::Merged => "merged",
            State::Archived => "archived",
        };
        f.write_str(state)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Filter {
    state: Option<State>,
    authored: bool,
    authors: Vec<Did>,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            state: Some(State::default()),
            authored: false,
            authors: vec![],
        }
    }
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

    pub fn matches(&self, profile: &Profile, patch: &Patch) -> bool {
        let matches_state = match self.state {
            Some(State::Draft) => matches!(patch.state(), patch::State::Draft),
            Some(State::Open) => matches!(patch.state(), patch::State::Open { .. }),
            Some(State::Merged) => matches!(patch.state(), patch::State::Merged { .. }),
            Some(State::Archived) => matches!(patch.state(), patch::State::Archived),
            None => true,
        };

        let matches_authored = self
            .authored
            .then(|| *patch.author().id() == profile.did())
            .unwrap_or(true);

        let matches_authors = (!self.authors.is_empty())
            .then(|| {
                self.authors
                    .iter()
                    .any(|other| *patch.author().id() == *other)
            })
            .unwrap_or(true);

        matches_state && matches_authored && matches_authors
    }
}

impl ToString for Filter {
    fn to_string(&self) -> String {
        let mut filter = String::new();

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

pub fn all(profile: &Profile, repository: &Repository) -> Result<Vec<(PatchId, Patch)>> {
    let cache = profile.patches(repository)?;
    let patches = cache.list()?;

    Ok(patches.flatten().collect())
}

pub fn find(profile: &Profile, repository: &Repository, id: &PatchId) -> Result<Option<Patch>> {
    let cache = profile.patches(repository)?;
    Ok(cache.get(id)?)
}
