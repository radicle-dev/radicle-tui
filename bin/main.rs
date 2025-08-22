mod cob;
mod commands;
mod git;
mod log;
mod settings;
mod state;
mod terminal;
#[cfg(test)]
mod test;
mod ui;

use std::env::args_os;
use std::ffi::OsString;
use std::io;
use std::{iter, process};

use thiserror::Error;

use radicle::version::Version;

use radicle_cli::terminal as cli_term;

use commands::*;
use terminal as term;

use crate::terminal::ForwardError;

pub const NAME: &str = "rad-tui";
pub const DESCRIPTION: &str = "Radicle terminal interfaces";
pub const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GIT_HEAD: &str = env!("GIT_HEAD");
pub const TIMESTAMP: &str = env!("GIT_COMMIT_TIME");
pub const VERSION: Version = Version {
    name: NAME,
    version: PKG_VERSION,
    commit: GIT_HEAD,
    timestamp: TIMESTAMP,
};

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Forward(#[from] term::ForwardError),
    #[error("{0}")]
    Args(#[from] lexopt::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

#[derive(Debug)]
enum CommandName {
    Other(Vec<OsString>),
    Help,
    Version,
}

#[derive(Default, Debug, PartialEq)]
struct OtherOptions {
    args: Vec<OsString>,
    forward: bool,
}

#[derive(Debug, PartialEq)]
enum Command {
    Other { opts: OtherOptions },
    Help,
    Version { json: bool },
}

fn main() {
    let args = args_os().collect::<Vec<_>>();

    match parse_args(&args[1..]).and_then(run) {
        Ok(_) => process::exit(0),
        Err(err) => {
            match err {
                // Do not print an additonal error message if `rad` itself
                // already printed its error(s).
                Error::Forward(ForwardError::RadInternal) => {}
                _ => radicle_term::error(format!("rad-tui: {err}")),
            }
            process::exit(1);
        }
    }
}

fn parse_args(args: &[OsString]) -> anyhow::Result<Command, Error> {
    use lexopt::prelude::*;

    let mut parser = lexopt::Parser::from_args(args);
    let mut command = None;
    let mut forward = true;
    let mut json = false;

    while let Some(arg) = parser.next()? {
        match arg {
            Long("no-forward") => {
                forward = false;
            }
            Long("json") => {
                json = true;
            }
            Long("help") | Short('h') => {
                command = Some(CommandName::Help);
            }
            Long("version") => {
                command = Some(CommandName::Version);
            }
            Value(val) if command.is_none() => {
                command = match val.to_string_lossy().as_ref() {
                    "help" => Some(CommandName::Help),
                    "version" => Some(CommandName::Version),
                    _ => {
                        let args = iter::once(val)
                            .chain(iter::from_fn(|| parser.value().ok()))
                            .collect();

                        Some(CommandName::Other(args))
                    }
                }
            }
            _ => return Err(arg.unexpected().into()),
        }
    }

    let command = match command {
        Some(CommandName::Help) => {
            if forward {
                Command::Other {
                    opts: OtherOptions {
                        args: vec!["help".into()],
                        forward,
                    },
                }
            } else {
                Command::Help
            }
        }
        Some(CommandName::Version) => {
            if forward {
                Command::Other {
                    opts: OtherOptions {
                        args: vec!["version".into()],
                        forward,
                    },
                }
            } else {
                Command::Version { json }
            }
        }
        Some(CommandName::Other(args)) => Command::Other {
            opts: OtherOptions { args, forward },
        },
        _ => Command::Other {
            opts: OtherOptions {
                args: vec![],
                forward,
            },
        },
    };

    Ok(command)
}

fn print_help() -> anyhow::Result<()> {
    println!("{DESCRIPTION}");
    println!();

    tui_help::run(Default::default(), cli_term::DefaultContext)
}

fn run(command: Command) -> Result<(), Error> {
    match command {
        Command::Version { json } => {
            let mut stdout = io::stdout();
            if json {
                VERSION.write_json(&mut stdout)?;
                println!();
            } else {
                println!("rad-tui {} ({})", VERSION.version, VERSION.commit);
            }
        }
        Command::Help => {
            print_help()?;
        }
        Command::Other { opts } => {
            let exe = opts.args.first();

            if let Some(exe) = exe.map(|s| s.to_str()) {
                run_other(exe, &opts.args[1..])?;
            } else if opts.forward {
                run_other(None, &[])?;
            } else {
                print_help()?;
            }
        }
    }

    Ok(())
}

fn run_other(command: Option<&str>, args: &[OsString]) -> Result<(), Error> {
    match command {
        Some("issue") => {
            term::run_command_args::<tui_issue::Options, _>(
                tui_issue::HELP,
                tui_issue::run,
                args.to_vec(),
            );
        }
        Some("patch") => {
            term::run_command_args::<tui_patch::Options, _>(
                tui_patch::HELP,
                tui_patch::run,
                args.to_vec(),
            );
        }
        Some("inbox") => {
            term::run_command_args::<tui_inbox::Options, _>(
                tui_inbox::HELP,
                tui_inbox::run,
                args.to_vec(),
            );
        }
        command => term::run_rad(command, args).map_err(|err| err.into()),
    }
}

#[cfg(test)]
mod cli {
    use crate::{parse_args, OtherOptions};

    #[test]
    fn empty_command_should_be_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let args = vec![];
        let expected = super::Command::Other {
            opts: OtherOptions {
                args: args.clone(),
                forward: true,
            },
        };

        let actual = parse_args(&args)?;
        assert_eq!(actual, expected);

        Ok(())
    }

    #[test]
    fn empty_command_should_not_be_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let args = vec!["--no-forward".into()];
        let expected = super::Command::Other {
            opts: OtherOptions::default(),
        };

        let actual = parse_args(&args)?;
        assert_eq!(actual, expected);

        Ok(())
    }

    #[test]
    fn version_command_should_be_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let args = vec!["version".into()];
        let expected = super::Command::Other {
            opts: OtherOptions {
                args: args.clone(),
                forward: true,
            },
        };

        let actual = parse_args(&args)?;
        assert_eq!(actual, expected);

        Ok(())
    }

    #[test]
    fn version_command_should_not_be_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let args = vec!["version".into(), "--no-forward".into()];
        let expected = super::Command::Version { json: false };

        let actual = parse_args(&args)?;
        assert_eq!(actual, expected);

        Ok(())
    }

    #[test]
    fn version_command_should_print_json() -> Result<(), Box<dyn std::error::Error>> {
        let args = vec!["version".into(), "--no-forward".into(), "--json".into()];
        let expected = super::Command::Version { json: true };

        let actual = parse_args(&args)?;
        assert_eq!(actual, expected);

        Ok(())
    }

    #[test]
    fn help_command_should_be_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let args = vec!["help".into()];
        let expected = super::Command::Other {
            opts: OtherOptions {
                args: args.clone(),
                forward: true,
            },
        };

        let actual = parse_args(&args)?;
        assert_eq!(actual, expected);

        Ok(())
    }

    #[test]
    fn help_command_should_not_be_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let args = vec!["help".into(), "--no-forward".into()];

        let actual = parse_args(&args)?;
        assert!(matches!(actual, super::Command::Help));

        Ok(())
    }
}
