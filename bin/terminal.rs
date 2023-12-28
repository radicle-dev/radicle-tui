pub mod args;
pub mod io;

use std::ffi::OsString;
use std::process;

pub use args::{Args, Error, Help};

use radicle_term as term;

use radicle::profile::Profile;

/// Context passed to all commands.
pub trait Context {
    /// Return the currently active profile, or an error if no profile is active.
    fn profile(&self) -> Result<Profile, anyhow::Error>;
}

impl Context for Profile {
    fn profile(&self) -> Result<Profile, anyhow::Error> {
        Ok(self.clone())
    }
}

impl<F> Context for F
where
    F: Fn() -> Result<Profile, anyhow::Error>,
{
    fn profile(&self) -> Result<Profile, anyhow::Error> {
        self()
    }
}

/// A command that can be run.
pub trait Command<A: Args, C: Context> {
    /// Run the command, given arguments and a context.
    fn run(self, args: A, context: C) -> anyhow::Result<()>;
}

impl<F, A: Args, C: Context> Command<A, C> for F
where
    F: FnOnce(A, C) -> anyhow::Result<()>,
{
    fn run(self, args: A, context: C) -> anyhow::Result<()> {
        self(args, context)
    }
}

#[allow(dead_code)]
pub fn run_command_args<A, C>(help: Help, cmd: C, args: Vec<OsString>) -> !
where
    A: Args,
    C: Command<A, fn() -> anyhow::Result<Profile>>,
{
    let options = match A::from_args(args) {
        Ok((opts, unparsed)) => {
            if let Err(err) = args::finish(unparsed) {
                term::error(err);
                process::exit(1);
            }
            opts
        }
        Err(err) => {
            let hint = match err.downcast_ref::<Error>() {
                Some(Error::Help) => {
                    term::help(help.name, help.version, help.description, help.usage);
                    process::exit(0);
                }
                Some(Error::HelpManual { name }) => {
                    let Ok(status) = term::manual(name) else {
                        term::error(format!("rad-tui {}: failed to load manual page", help.name));
                        process::exit(1);
                    };
                    process::exit(status.code().unwrap_or(0));
                }
                Some(Error::Usage) => {
                    term::usage(help.name, help.usage);
                    process::exit(1);
                }
                Some(Error::WithHint { hint, .. }) => Some(hint),
                None => None,
            };
            term::error(format!("rad-tui {}: {err}", help.name));

            if let Some(hint) = hint {
                term::hint(hint);
            }
            process::exit(1);
        }
    };

    match cmd.run(options, self::profile) {
        Ok(()) => process::exit(0),
        Err(err) => {
            fail(help.name, &err);
            process::exit(1);
        }
    }
}

/// Get the default profile. Fails if there is no profile.
pub fn profile() -> Result<Profile, anyhow::Error> {
    match Profile::load() {
        Ok(profile) => Ok(profile),
        Err(radicle::profile::Error::NotFound(path)) => Err(args::Error::WithHint {
            err: anyhow::anyhow!("Radicle profile not found in '{}'.", path.display()),
            hint: "To setup your radicle profile, run `rad auth`.",
        }
        .into()),
        Err(radicle::profile::Error::Config(e)) => Err(e.into()),
        Err(e) => Err(anyhow::anyhow!("Could not load radicle profile: {e}")),
    }
}

pub fn fail(_name: &str, error: &anyhow::Error) {
    let err = error.to_string();
    let err = err.trim_end();

    for line in err.lines() {
        term::error(line);
    }

    if let Some(Error::WithHint { hint, .. }) = error.downcast_ref::<Error>() {
        term::hint(hint);
    }
}
