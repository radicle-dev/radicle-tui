use std::fmt;
use std::fmt::Write as _;

use anyhow::Result;

use radicle::cob::patch::{Patch, PatchId};
use radicle::identity::Did;
use radicle::node::device::Device;
use radicle::patch::cache::Patches;
use radicle::patch::{Review, ReviewId, Revision, Status};
use radicle::storage::git::Repository;
use radicle::Profile;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Filter {
    status: Option<Status>,
    authored: bool,
    authors: Vec<Did>,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            status: Some(Status::default()),
            authored: false,
            authors: vec![],
        }
    }
}

impl Filter {
    pub fn with_status(mut self, status: Option<Status>) -> Self {
        self.status = status;
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
}

impl fmt::Display for Filter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(state) = &self.status {
            write!(f, "is:{}", state)?;
            f.write_char(' ')?;
        }
        if self.authored {
            f.write_str("is:authored")?;
            f.write_char(' ')?;
        }
        if !self.authors.is_empty() {
            f.write_str("authors:")?;
            f.write_char('[')?;

            let mut authors = self.authors.iter().peekable();
            while let Some(author) = authors.next() {
                f.write_str(&author.encode())?;

                if authors.peek().is_some() {
                    f.write_char(',')?;
                }
            }
            f.write_char(']')?;
        }

        Ok(())
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

pub fn find_review<'a, G>(
    patch: &'a Patch,
    revision: &Revision,
    signer: &Device<G>,
) -> Option<(ReviewId, &'a Review)> {
    patch
        .reviews_of(revision.id())
        .find(|(_, review)| review.author().public_key() == signer.public_key())
        .map(|(id, review)| (*id, review))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use anyhow::Result;
    use radicle::patch;

    use super::*;

    #[test]
    fn patch_filter_display_with_status_should_succeed() -> Result<()> {
        let actual = Filter::default().with_status(Some(patch::Status::Open));

        assert_eq!(String::from("is:open "), actual.to_string());

        Ok(())
    }

    #[test]
    fn patch_filter_display_with_status_and_authored_should_succeed() -> Result<()> {
        let actual = Filter::default()
            .with_status(Some(patch::Status::Open))
            .with_authored(true);

        assert_eq!(String::from("is:open is:authored "), actual.to_string());

        Ok(())
    }

    #[test]
    fn patch_filter_display_with_status_and_author_should_succeed() -> Result<()> {
        let actual = Filter::default()
            .with_status(Some(patch::Status::Open))
            .with_author(Did::from_str(
                "did:key:z6MkswQE8gwZw924amKatxnNCXA55BMupMmRg7LvJuim2C1V",
            )?);

        assert_eq!(
            String::from(
                "is:open authors:[did:key:z6MkswQE8gwZw924amKatxnNCXA55BMupMmRg7LvJuim2C1V]"
            ),
            actual.to_string()
        );

        Ok(())
    }
}
