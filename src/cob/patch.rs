use std::str::FromStr;

use anyhow::{anyhow, Result};

use radicle::cob::patch::{Patch, PatchId, Patches};
use radicle::identity::Did;
use radicle::storage::git::Repository;

const STATE_PROP: &'static str = "state:";
const BOOL_PROP: &'static str = "is:";
const AUTHOR_PROP: &'static str = "authors:";

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

#[derive(thiserror::Error, Debug)]
pub enum FilterError {
    ///
    #[error("unknown property found: {property}")]
    UnknownProperty { property: String },
    ///
    #[error("unknown value for property 'state': {value}")]
    UnknownStateValue { value: String },
    ///
    #[error("unknown value for property 'is': {value}")]
    UnknownBoolValue { value: String },
    ///
    #[error("unknown value for property 'authors': {value}")]
    UnknownAuthorsValue { value: String },
    ///
    #[error("Did parsing failed: {err}")]
    DidFormat { err: anyhow::Error },
}

impl FromStr for Filter {
    type Err = FilterError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut filter = Filter::default();

        let parts = s.split(' ').collect::<Vec<_>>();
        for part in parts {
            if part.starts_with(STATE_PROP) {
                let state = part.replace(STATE_PROP, "");
                match state.as_str() {
                    "draft" => filter = filter.with_state(State::Draft),
                    "open" => filter = filter.with_state(State::Open),
                    "merged" => filter = filter.with_state(State::Merged),
                    "archived" => filter = filter.with_state(State::Archived),
                    other => {
                        return Err(FilterError::UnknownStateValue {
                            value: other.to_string(),
                        })
                    }
                }
            }
            if part.starts_with(BOOL_PROP) {
                let is = part.replace(BOOL_PROP, "");
                match is.as_str() {
                    "authored" => filter = filter.with_authored(true),
                    other => {
                        return Err(FilterError::UnknownBoolValue {
                            value: other.to_string(),
                        })
                    }
                }
            }
            if part.starts_with(AUTHOR_PROP) {
                let authors = part.replace(AUTHOR_PROP, "");
                if let Some(list) = authors.strip_prefix('[') {
                    if let Some(list) = list.strip_suffix(']') {
                        match super::parse_dids(list.to_string()) {
                            Ok(dids) => {
                                for did in dids {
                                    filter = filter.with_author(did);
                                }
                            }
                            Err(err) => return Err(FilterError::DidFormat { err }),
                        }
                    } else {
                        return Err(FilterError::UnknownAuthorsValue {
                            value: authors.to_string(),
                        });
                    }
                } else {
                    return Err(FilterError::UnknownAuthorsValue {
                        value: authors.to_string(),
                    });
                }
            }
        }

        Ok(filter)
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
