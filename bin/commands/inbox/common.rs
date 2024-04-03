use std::fmt::Display;

use serde::Serialize;

use radicle::identity::RepoId;

/// The application's subject. It tells the application
/// which widgets to render and which output to produce.
///
/// Depends on CLI arguments given by the user.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum SelectionMode {
    Id,
    #[default]
    Operation,
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub enum RepositoryMode {
    #[default]
    Contextual,
    All,
    ByRepo((RepoId, Option<String>)),
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Mode {
    selection: SelectionMode,
    repository: RepositoryMode,
}

impl Mode {
    pub fn with_selection(mut self, selection: SelectionMode) -> Self {
        self.selection = selection;
        self
    }

    pub fn with_repository(mut self, repository: RepositoryMode) -> Self {
        self.repository = repository;
        self
    }

    pub fn selection(&self) -> &SelectionMode {
        &self.selection
    }

    pub fn repository(&self) -> &RepositoryMode {
        &self.repository
    }
}

/// The selected issue operation returned by the operation
/// selection widget.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum InboxOperation {
    Show,
    Clear,
}

impl Display for InboxOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InboxOperation::Show => {
                write!(f, "show")
            }
            InboxOperation::Clear => {
                write!(f, "clear")
            }
        }
    }
}
