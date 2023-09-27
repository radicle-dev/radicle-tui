use radicle_term as term;

use radicle::cob::issue::{Issue, IssueId};
use radicle::cob::patch::{Patch, PatchId};
use radicle::crypto::ssh::keystore::MemorySigner;
use radicle::prelude::{Id, Project, Signer};
use radicle::profile::env::RAD_PASSPHRASE;
use radicle::Profile;

use radicle::storage::git::Repository;
use radicle::storage::ReadStorage;

use term::{passphrase, spinner};

pub struct Context {
    profile: Profile,
    id: Id,
    project: Project,
    repository: Repository,
    issues: Vec<(IssueId, Issue)>,
    patches: Vec<(PatchId, Patch)>,
    signer: Box<dyn Signer>,
}

impl Context {
    pub fn new(id: Id) -> Result<Self, anyhow::Error> {
        use anyhow::Context;
        
        let profile = profile()?;
        let signer = signer(&profile)?;
        let payload = &profile
            .storage
            .get(signer.public_key(), id)?
            .context("No project with such `id` exists")?;
        let project = payload.project()?;

        let repository = profile.storage.repository(id).unwrap();
        let issues = crate::cob::issue::all(&repository).unwrap_or_default();
        let patches = crate::cob::patch::all(&repository).unwrap_or_default();

        Ok(Self {
            id,
            profile,
            project,
            repository,
            issues,
            patches,
            signer,
        })
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

    #[allow(clippy::borrowed_box)]
    pub fn signer(&self) -> &Box<dyn Signer> {
        &self.signer
    }

    pub fn reload(&mut self) {
        self.issues = crate::cob::issue::all(&self.repository).unwrap_or_default();
        self.patches = crate::cob::patch::all(&self.repository).unwrap_or_default();
    }
}

/// Get the default profile. Fails if there is no profile.
fn profile() -> Result<Profile, anyhow::Error> {
    match Profile::load() {
        Ok(profile) => Ok(profile),
        Err(_) => Err(anyhow::anyhow!(
            "Could not load radicle profile. To setup your radicle profile, run `rad auth`."
        )),
    }
}

/// Get the signer. First we try getting it from ssh-agent, otherwise we prompt the user.
fn signer(profile: &Profile) -> anyhow::Result<Box<dyn Signer>> {
    if let Ok(signer) = profile.signer() {
        return Ok(signer);
    }
    let passphrase = passphrase(RAD_PASSPHRASE)?;
    let spinner = spinner("Unsealing key...");
    let signer = MemorySigner::load(&profile.keystore, Some(passphrase))?;

    spinner.finish();

    Ok(signer.boxed())
}
