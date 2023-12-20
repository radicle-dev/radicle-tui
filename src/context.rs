use radicle_term as term;

use radicle::cob::issue::{Issue, IssueId};
use radicle::cob::patch::{Patch, PatchId};
use radicle::crypto::ssh::keystore::{Keystore, MemorySigner};
use radicle::crypto::Signer;
use radicle::prelude::{Id, Project};
use radicle::profile::env::RAD_PASSPHRASE;
use radicle::storage::ReadRepository;
use radicle::Profile;

use radicle::storage::git::Repository;
use radicle::storage::ReadStorage;

use term::{passphrase, spinner, Passphrase};

use inquire::validator;

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
        let profile = profile()?;
        let signer = signer(&profile)?;

        let repository = profile.storage.repository(id).unwrap();
        let doc = repository.identity_doc()?;
        let project = doc.project()?;

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
