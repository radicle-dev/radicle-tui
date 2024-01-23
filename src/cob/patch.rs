use anyhow::Result;

use radicle::cob::patch::{Patch, PatchId, Patches};
use radicle::identity::Did;
use radicle::storage::git::Repository;

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub enum State {
    Draft,
    #[default]
    Open,
    Merged,
    Archived,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct Filter {
    state: Option<State>,
    authored: bool,
    authors: Vec<Did>,
}

impl Filter {
    pub fn with_state(mut self, state: State) -> Self {
        self.state = Some(state);
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

    pub fn matches(&self, _patch: &Patch) -> bool {
        true
    }
}

pub fn all(repository: &Repository) -> Result<Vec<(PatchId, Patch)>> {
    let patches = Patches::open(repository)?
        .all()
        .map(|iter| iter.flatten().collect::<Vec<_>>())?;

    Ok(patches
        .into_iter()
        .map(|(id, patch)| (id, patch))
        .collect::<Vec<_>>())
}

pub fn find(repository: &Repository, id: &PatchId) -> Result<Option<Patch>> {
    let patches = Patches::open(repository)?;
    Ok(patches.get(id)?)
}
