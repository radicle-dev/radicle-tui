use radicle::node::device::Device;
use radicle::patch::{Patch, Review, ReviewId, Revision};

pub mod issue;

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
