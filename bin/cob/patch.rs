use anyhow::Result;

use radicle::cob::patch::{Patch, PatchId};
use radicle::node::device::Device;
use radicle::patch::cache::Patches;
use radicle::patch::{Review, ReviewId, Revision};
use radicle::storage::git::Repository;
use radicle::Profile;

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
