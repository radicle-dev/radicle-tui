use std::fmt::Display;

use radicle_term as term;

use radicle::cob::issue::{Issue, IssueId};
use radicle::cob::patch::{Patch, PatchId};
use radicle::crypto::ssh::keystore::{Keystore, MemorySigner};
use radicle::crypto::Signer;
use radicle::identity::{Id, Project};
use radicle::profile::env::RAD_PASSPHRASE;
use radicle::storage::git::Repository;
use radicle::storage::{ReadRepository, ReadStorage};

use radicle::Profile;

use term::{passphrase, spinner, Passphrase};

use inquire::validator;

/// Git revision parameter. Supports extended SHA-1 syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rev(String);

impl From<String> for Rev {
    fn from(value: String) -> Self {
        Rev(value)
    }
}

impl Display for Rev {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Application context that holds all the project data that are
/// needed to render it.
pub struct Context {
    profile: Profile,
    id: Id,
    project: Project,
    repository: Repository,
    issues: Option<Vec<(IssueId, Issue)>>,
    patches: Option<Vec<(PatchId, Patch)>>,
    signer: Option<Box<dyn Signer>>,
}

impl Context {
    pub fn new(profile: Profile, id: Id) -> Result<Self, anyhow::Error> {
        let repository = profile.storage.repository(id).unwrap();
        let project = repository.identity_doc()?.project()?;
        let issues = None;
        let patches = None;
        let signer = None;

        Ok(Self {
            profile,
            id,
            project,
            repository,
            issues,
            patches,
            signer,
        })
    }

    pub fn with_issues(mut self) -> Self {
        use crate::cob::issue;
        self.issues = Some(issue::all(&self.repository).unwrap_or_default());
        self
    }

    pub fn with_patches(mut self) -> Self {
        use crate::cob::patch;
        self.patches = Some(patch::all(&self.repository).unwrap_or_default());
        self
    }

    pub fn with_signer(mut self) -> Self {
        self.signer = signer(&self.profile).ok();
        self
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

    pub fn issues(&self) -> &Option<Vec<(IssueId, Issue)>> {
        &self.issues
    }

    pub fn patches(&self) -> &Option<Vec<(PatchId, Patch)>> {
        &self.patches
    }

    #[allow(clippy::borrowed_box)]
    pub fn signer(&self) -> &Option<Box<dyn Signer>> {
        &self.signer
    }

    // pub fn reload(&mut self) {
    //     use crate::cob::issue;
    //     use crate::cob::patch;

    //     if self.issues.is_some() {
    //         self.issues = Some(issue::all(&self.repository).unwrap_or_default());
    //     }
    //     if self.patches.is_some() {
    //         self.patches = Some(patch::all(&self.repository).unwrap_or_default());
    //     }
    // }

    pub fn reload_patches(&mut self) {
        use crate::cob::patch;
        self.patches = Some(patch::all(&self.repository).unwrap_or_default());
    }

    pub fn reload_issues(&mut self) {
        use crate::cob::issue;
        self.issues = Some(issue::all(&self.repository).unwrap_or_default());
    }
}

/// Validates secret key passphrases.
#[derive(Clone)]
pub struct PassphraseValidator {
    keystore: Keystore,
}

impl PassphraseValidator {
    /// Create a new validator.
    pub fn new(keystore: Keystore) -> Self {
        Self { keystore }
    }
}

impl inquire::validator::StringValidator for PassphraseValidator {
    fn validate(
        &self,
        input: &str,
    ) -> Result<validator::Validation, inquire::error::CustomUserError> {
        let passphrase = Passphrase::from(input.to_owned());
        if self.keystore.is_valid_passphrase(&passphrase)? {
            Ok(validator::Validation::Valid)
        } else {
            Ok(validator::Validation::Invalid(
                validator::ErrorMessage::from("Invalid passphrase, please try again"),
            ))
        }
    }
}

/// Get the signer. First we try getting it from ssh-agent, otherwise we prompt the user,
/// if we're connected to a TTY.
pub fn signer(profile: &Profile) -> anyhow::Result<Box<dyn Signer>> {
    if let Ok(signer) = profile.signer() {
        return Ok(signer);
    }
    let validator = PassphraseValidator::new(profile.keystore.clone());
    let passphrase = match passphrase(validator) {
        Ok(p) => p,
        Err(inquire::InquireError::NotTTY) => {
            return Err(anyhow::anyhow!(
                "running in non-interactive mode, please set `{RAD_PASSPHRASE}` to unseal your key",
            ));
        }
        Err(e) => return Err(e.into()),
    };
    let spinner = spinner("Unsealing key...");
    let signer = MemorySigner::load(&profile.keystore, Some(passphrase))?;

    spinner.finish();

    Ok(signer.boxed())
}
