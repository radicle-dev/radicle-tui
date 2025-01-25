use std::ffi::OsString;

use radicle_cli::terminal as cli_term;
use radicle_term as term;

use cli_term::args::{Args, Error, Help};
use cli_term::Context;

use super::*;

pub const HELP: Help = Help {
    name: "help",
    description: "TUI help",
    version: env!("CARGO_PKG_VERSION"),
    usage: "Usage: rad-tui help [--help]",
};

const COMMANDS: &[Help] = &[tui_help::HELP];

#[derive(Default)]
pub struct Options {}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        let mut parser = lexopt::Parser::from_args(args);

        if let Some(arg) = parser.next()? {
            return Err(anyhow::anyhow!(arg.unexpected()));
        }
        Err(Error::HelpManual { name: "rad-tui" }.into())
    }
}

pub fn run(_options: Options, ctx: impl Context) -> anyhow::Result<()> {
    term::print("Usage: rad-tui <command> [--help]");

    if let Err(e) = ctx.profile() {
        term::blank();
        match e.downcast_ref() {
            Some(Error::WithHint { err, hint }) => {
                term::print(term::format::yellow(err));
                term::print(term::format::yellow(hint));
            }
            Some(e) => {
                term::error(e);
            }
            None => {
                term::error(e);
            }
        }
        term::blank();
    }

    term::print("Common `rad-tui` commands used in various situations:");
    term::blank();

    for help in COMMANDS {
        term::info!(
            "\t{} {}",
            term::format::bold(format!("{:-12}", help.name)),
            term::format::dim(help.description)
        );
    }
    term::blank();
    term::print("See `rad-tui <command> --help` to learn about a specific command.");
    term::blank();

    Ok(())
}
