use std::ffi::OsString;

use radicle_cli::terminal as cli_term;
use radicle_term as term;

use cli_term::args::{Args, Error, Help};
use cli_term::Context;

use super::*;

pub const HELP: Help = Help {
    name: "help",
    description: "Print help",
    version: env!("CARGO_PKG_VERSION"),
    usage: "Usage: rad-tui help [--help]",
};

const COMMANDS: &[Help] = &[tui_help::HELP];

#[derive(Default)]
pub struct Options {}

impl Args for Options {
    fn from_args(_args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        Err(Error::HelpManual { name: "rad-tui" }.into())
    }
}

pub fn run(_options: Options, ctx: impl Context) -> anyhow::Result<()> {
    println!(
        "{} {}",
        term::format::secondary("Usage:").bold(),
        term::format::tertiary("rad-tui [COMMAND] [OPTIONS]"),
    );

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

    term::blank();
    println!("{}", term::format::secondary("Options:").bold(),);
    term::info!(
        "\t{} {}",
        term::format::tertiary(format!("{:-16}", "--no-forward")),
        term::format::default("Don't forward command to `rad` (default: false)")
    );
    term::info!(
        "\t{} {}",
        term::format::tertiary(format!("{:-16}", "--json")),
        term::format::default("Print version as JSON")
    );
    term::info!(
        "\t{} {}",
        term::format::tertiary(format!("{:-16}", "--version")),
        term::format::default("Print version")
    );
    term::info!(
        "\t{} {}",
        term::format::tertiary(format!("{:-16}", "--help")),
        term::format::default("Print command specific help")
    );

    term::blank();
    println!("{}", term::format::secondary("Commands:").bold(),);

    term::info!(
        "\t{} {}",
        term::format::tertiary(format!("{:-16}", "version")),
        term::format::default("Print version")
    );
    for help in COMMANDS {
        term::info!(
            "\t{} {}",
            term::format::tertiary(format!("{:-16}", help.name)),
            term::format::default(help.description)
        );
    }

    term::blank();
    println!(
        "See {} to learn about a specific command.",
        term::format::tertiary("`rad-tui <command> --help`")
    );
    term::blank();

    Ok(())
}
