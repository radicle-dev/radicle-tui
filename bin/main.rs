mod cob;
mod commands;
mod git;
mod log;
mod settings;
#[cfg(test)]
mod test;
mod ui;

use std::ffi::OsString;
use std::io;
use std::io::Write;
use std::{iter, process};

use anyhow::anyhow;

use radicle::version::Version;

use radicle_cli::terminal;
use radicle_term as term;

use commands::*;

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
enum Command {
    Other(Vec<OsString>),
    Help,
    Version,
}

fn main() {
    match parse_args().map_err(Some).and_then(run) {
        Ok(_) => process::exit(0),
        Err(err) => {
            if let Some(err) = err {
                term::error(format!("rad: {err}"));
            }
            process::exit(1);
        }
    }
}

fn parse_args() -> anyhow::Result<Command> {
    use lexopt::prelude::*;

    let mut parser = lexopt::Parser::from_env();
    let mut command = None;

    while let Some(arg) = parser.next()? {
        match arg {
            Long("help") | Short('h') => {
                command = Some(Command::Help);
            }
            Long("version") => {
                command = Some(Command::Version);
            }
            Value(val) if command.is_none() => {
                let args = iter::once(val)
                    .chain(iter::from_fn(|| parser.value().ok()))
                    .collect();

                command = Some(Command::Other(args))
            }
            _ => return Err(anyhow::anyhow!(arg.unexpected())),
        }
    }

    Ok(command.unwrap_or_else(|| Command::Other(vec![])))
}

fn print_help() -> anyhow::Result<()> {
    VERSION.write(&mut io::stdout())?;
    println!("{DESCRIPTION}");
    println!();

    tui_help::run(Default::default(), terminal::DefaultContext)
}

fn run(command: Command) -> Result<(), Option<anyhow::Error>> {
    match command {
        Command::Version => {
            let mut stdout = io::stdout();
            VERSION
                .write_json(&mut stdout)
                .map_err(|e| Some(e.into()))?;
            writeln!(&mut stdout).ok();
        }
        Command::Help => {
            print_help()?;
        }
        Command::Other(args) => {
            let exe = args.first();

            if let Some(Some(exe)) = exe.map(|s| s.to_str()) {
                run_other(exe, &args[1..])?;
            } else {
                print_help()?;
            }
        }
    }

    Ok(())
}

fn run_other(exe: &str, args: &[OsString]) -> Result<(), Option<anyhow::Error>> {
    match exe {
        "issue" => {
            terminal::run_command_args::<tui_issue::Options, _>(
                tui_issue::HELP,
                tui_issue::run,
                args.to_vec(),
            );
        }
        "patch" => {
            terminal::run_command_args::<tui_patch::Options, _>(
                tui_patch::HELP,
                tui_patch::run,
                args.to_vec(),
            );
        }
        "inbox" => {
            terminal::run_command_args::<tui_inbox::Options, _>(
                tui_inbox::HELP,
                tui_inbox::run,
                args.to_vec(),
            );
        }
        other => Err(Some(anyhow!(
            "`rad-tui {other}` is not a command. See `rad-tui --help` for a list of commands.",
        ))),
    }
}
