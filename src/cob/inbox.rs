use anyhow::Result;

use radicle::node::notifications::Notification;
use radicle::storage::git::Repository;
use radicle::Profile;

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct Filter {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortBy {
    pub reverse: bool,
    pub field: &'static str,
}

impl Default for SortBy {
    fn default() -> Self {
        Self {
            reverse: true,
            field: "timestamp",
        }
    }
}

pub fn all(repository: &Repository, profile: &Profile) -> Result<Vec<Notification>> {
    let all = profile
        .notifications_mut()?
        .by_repo(&repository.id, "timestamp")?
        .collect::<Vec<_>>();

    let mut notifications = vec![];
    for n in all {
        let n = n?;
        notifications.push(n);
    }

    Ok(notifications)
}
