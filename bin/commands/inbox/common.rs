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
