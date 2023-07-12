use radicle::cob::issue::{Issue, IssueId};
use radicle::cob::patch::{Patch, PatchId};
use radicle::prelude::{Id, Project};
use radicle::Profile;

use radicle::storage::git::Repository;
use radicle::storage::ReadStorage;
pub struct Context {
    profile: Profile,
    id: Id,
    project: Project,
    repository: Repository,
    issues: Vec<(IssueId, Issue)>,
    patches: Vec<(PatchId, Patch)>,
}

impl Context {
    pub fn new(profile: Profile, id: Id, project: Project) -> Self {
        let repository = profile.storage.repository(id).unwrap();
        let issues = crate::cob::issue::all(&repository).unwrap_or_default();
        let patches = crate::cob::patch::all(&repository).unwrap_or_default();

        Self {
            id,
            profile,
            project,
            repository,
            issues,
            patches,
        }
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn repository(&self) -> &Repository {
        &self.repository
    }

    pub fn issues(&self) -> &Vec<(IssueId, Issue)> {
        &self.issues
    }

    pub fn patches(&self) -> &Vec<(PatchId, Patch)> {
        &self.patches
    }
}
