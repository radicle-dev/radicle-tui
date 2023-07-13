use std::str::FromStr;

use anyhow::Result;
use radicle::cob::{ActorId, Tag};

pub mod issue;
pub mod patch;

pub fn parse_tags(input: String) -> Result<Vec<Tag>> {
    let mut tags = vec![];
    for name in input.split(',') {
        match Tag::new(name.trim()) {
            Ok(tag) => tags.push(tag),
            Err(err) => return Err(anyhow::anyhow!(err).context("Can't parse tags.")),
        }
    }

    Ok(tags)
}

pub fn parse_assignees(input: String) -> Result<Vec<ActorId>> {
    let mut assignees = vec![];
    for did in input.split(',') {
        match ActorId::from_str(did) {
            Ok(actor) => assignees.push(actor),
            Err(err) => return Err(anyhow::anyhow!(err).context("Can't parse assignees.")),
        }
    }

    Ok(assignees)
}
