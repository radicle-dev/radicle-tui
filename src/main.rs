use std::process;

use anyhow::{anyhow, Context};
use log::info;
use log::LevelFilter;

use radicle::profile;
use radicle::{
    crypto::ssh::keystore::MemorySigner, prelude::Signer, profile::env::RAD_PASSPHRASE, Profile,
};
use radicle_term as term;
use radicle_tui::Window;

use radicle::storage::ReadStorage;
use term::{passphrase, spinner};

mod issue;

pub const NAME: &str = "radicle-tui";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GIT_HEAD: &str = env!("GIT_HEAD");
pub const FPS: u64 = 60;

pub const HELP: &str = r#"
Usage

    radicle-tui [<option>...]

Options

    --version       Print version
    --help          Print help

"#;

struct Options;

impl Options {
    fn from_env() -> Result<Self, anyhow::Error> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_env();

        while let Some(arg) = parser.next()? {
            match arg {
                Long("version") => {
                    println!("{NAME} {VERSION}+{GIT_HEAD}");
                    process::exit(0);
                }
                Long("help") | Short('h') => {
                    println!("{HELP}");
                    process::exit(0);
                }
                _ => anyhow::bail!(arg.unexpected()),
            }
        }

        Ok(Self {})
    }
}

/// Get the default profile. Fails if there is no profile.
pub fn profile() -> Result<Profile, anyhow::Error> {
    match Profile::load() {
        Ok(profile) => Ok(profile),
        Err(_) => Err(anyhow::anyhow!(
            "Could not load radicle profile. To setup your radicle profile, run `rad auth`."
        )),
    }
}

/// Get the signer. First we try getting it from ssh-agent, otherwise we prompt the user.
pub fn signer(profile: &Profile) -> anyhow::Result<Box<dyn Signer>> {
    if let Ok(signer) = profile.signer() {
        return Ok(signer);
    }
    let passphrase = passphrase(RAD_PASSPHRASE)?;
    let spinner = spinner("Unsealing key...");
    let signer = MemorySigner::load(&profile.keystore, Some(passphrase))?;

    spinner.finish();

    Ok(signer.boxed())
}

fn execute() -> anyhow::Result<()> {
    let _ = Options::from_env()?;

    let (_, id) = radicle::rad::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;
    let profile = profile()?;
    let signer = signer(&profile)?;
    let payload = &profile
        .storage
        .get(signer.public_key(), id)?
        .context("No project with such `id` exists")?;
    let project = payload.project()?;

    let logfile = format!(
        "{}/radicle-tui.log",
        profile::home()?.path().to_string_lossy()
    );
    simple_logging::log_to_file(logfile, LevelFilter::Info)?;
    info!("Launching window...");

    let mut window = Window::default();
    window.run(
        &mut issue::App::new(profile, id, project, signer),
        1000 / FPS,
    )?;

    Ok(())
}

fn main() {
    if let Err(err) = execute() {
        term::error(format!("Error: radicle-tui: {err}"));
        process::exit(1);
    }
}
