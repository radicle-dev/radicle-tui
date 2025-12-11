use serde::Serialize;

use radicle::{identity::RepoId, node::notifications::NotificationId};

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub enum RepositoryMode {
    #[default]
    Contextual,
    All,
    ByRepo((RepoId, Option<String>)),
}

/// The selected issue operation returned by the operation
/// selection widget.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum InboxOperation {
    Show { id: NotificationId },
    Clear { id: NotificationId },
}
