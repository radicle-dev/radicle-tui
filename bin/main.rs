mod cob;
mod commands;
mod git;
mod log;
mod settings;
mod terminal;
#[cfg(test)]
mod test;
mod ui;

use std::ffi::OsString;
use std::io;
use std::io::Write;
use std::{iter, process};

use radicle::version::Version;

use radicle_cli::terminal as cli_term;

use commands::*;
use terminal as term;

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

#[derive(Debug)]
enum CommandName {
    Other(Vec<OsString>),
    Help,
    Version,
}

#[derive(Debug)]
struct OtherOptions {
    args: Vec<OsString>,
    forward: bool,
}

#[derive(Debug)]
enum Command {
    Other { opts: OtherOptions },
    Help,
    Version { json: bool },
}

fn main() {
    match parse_args().map_err(Some).and_then(run) {
        Ok(_) => process::exit(0),
        Err(err) => {
            if let Some(err) = err {
                radicle_term::error(format!("rad-tui: {err}"));
            }
            process::exit(1);
        }
    }
}

fn parse_args() -> anyhow::Result<Command> {
    use lexopt::prelude::*;

    let mut parser = lexopt::Parser::from_env();
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
            _ => return Err(anyhow::anyhow!(arg.unexpected())),
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

fn run(command: Command) -> Result<(), Option<anyhow::Error>> {
    match command {
        Command::Version { json } => {
            let mut stdout = io::stdout();
            if json {
                VERSION
                    .write_json(&mut stdout)
                    .map_err(|e| Some(e.into()))?;
                writeln!(&mut stdout).ok();
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

fn run_other(command: Option<&str>, args: &[OsString]) -> Result<(), Option<anyhow::Error>> {
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
        command => term::run_rad(command, args),
    }
}

#[cfg(test)]
mod cli {
    use assert_cmd::prelude::*;
    use predicates::prelude::*;
    use std::process::Command;

    mod assert {
        use predicates::prelude::*;
        use predicates::str::ContainsPredicate;

        pub fn is_rad_manual() -> ContainsPredicate {
            predicate::str::contains("Radicle CLI Manual")
        }

        pub fn is_rad_help() -> ContainsPredicate {
            predicate::str::contains("Radicle command line interface")
        }

        pub fn is_help() -> ContainsPredicate {
            predicate::str::contains("Radicle terminal interfaces")
        }
    }

    #[test]
    fn can_be_executed() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.assert().success();

        Ok(())
    }

    #[test]
    fn empty_command_is_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.assert().success().stdout(assert::is_rad_help());

        Ok(())
    }

    #[test]
    fn empty_command_is_not_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.arg("--no-forward");
        cmd.assert().success().stdout(assert::is_help());

        Ok(())
    }

    #[test]
    fn version_command_is_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.arg("version");
        cmd.assert()
            .success()
            .stdout(predicate::str::starts_with("rad "));

        Ok(())
    }

    #[test]
    fn version_command_is_not_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.arg("version").arg("--no-forward");
        cmd.assert()
            .success()
            .stdout(predicate::str::starts_with("rad-tui "));

        Ok(())
    }

    #[test]
    fn version_command_prints_json() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.arg("version").arg("--no-forward").arg("--json");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("\"name\":\"rad-tui\""));

        Ok(())
    }

    #[test]
    fn help_command_is_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.arg("help");
        cmd.assert().success().stdout(assert::is_rad_manual());

        Ok(())
    }

    #[test]
    fn help_command_is_not_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.arg("help").arg("--no-forward");
        cmd.assert().success().stdout(assert::is_help());

        Ok(())
    }
}
