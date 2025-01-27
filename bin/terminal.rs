use std::ffi::OsString;
use std::io::ErrorKind;
use std::process;

use anyhow::anyhow;

use radicle_cli::terminal;
use radicle_cli::terminal::args;
use radicle_cli::terminal::io;
use radicle_cli::terminal::{Args, Command, DefaultContext, Error, Help};

fn _run_rad(args: &[OsString]) -> Result<(), Option<anyhow::Error>> {
    let status = process::Command::new("rad").args(args).status();

    match status {
        Ok(status) => {
            if !status.success() {
                return Err(None);
            }
        }
        Err(err) => {
            if let ErrorKind::NotFound = err.kind() {
                return Err(Some(anyhow!("'rad' was not found.",)));
            } else {
                return Err(Some(err.into()));
            }
        }
    }

    Ok(())
}

pub fn run_rad(command: &str, args: &[OsString]) -> Result<(), Option<anyhow::Error>> {
    let args = [vec![command.into()], args.to_vec()].concat();
    
    _run_rad(&args)
}

pub fn run_command_args<A, C>(help: Help, cmd: C, args: Vec<OsString>) -> !
where
    A: Args,
    C: Command<A, DefaultContext>,
{
    use io as term;

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
                    help.print();
                    process::exit(0);
                }
                // Print the manual, or the regular help if there's an error.
                Some(Error::HelpManual { name }) => {
                    let Ok(status) = term::manual(name) else {
                        help.print();
                        process::exit(0);
                    };
                    if !status.success() {
                        help.print();
                        process::exit(0);
                    }
                    process::exit(status.code().unwrap_or(0));
                }
                Some(Error::Usage) => {
                    term::usage(help.name, help.usage);
                    process::exit(1);
                }
                Some(Error::WithHint { hint, .. }) => Some(hint),
                None => None,
            };
            io::error(format!("rad-tui {}: {err}", help.name));

            if let Some(hint) = hint {
                io::hint(hint);
            }
            process::exit(1);
        }
    };

    match cmd.run(options, DefaultContext) {
        Ok(()) => process::exit(0),
        Err(err) => {
            terminal::fail(help.name, &err);
            process::exit(1);
        }
    }
}
