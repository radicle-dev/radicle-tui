#[path = "patch/suite.rs"]
mod suite;

use std::ffi::OsString;

use anyhow::anyhow;

use crate::terminal;
use crate::terminal::args::{Args, Error, Help};

#[allow(dead_code)]
pub const HELP: Help = Help {
    name: "patch",
    description: "Terminal interfaces for patches",
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad-tui patch

General options

    --help               Print help
"#,
};

#[allow(dead_code)]
pub struct Options {
    op: Operation,
}

pub enum Operation {
    Suite,
}

#[derive(Default, PartialEq, Eq)]
pub enum OperationName {
    #[default]
    Suite,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let op: Option<OperationName> = None;

        #[allow(clippy::never_loop)]
        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") | Short('h') => {
                    return Err(Error::Help.into());
                }
                _ => return Err(anyhow!(arg.unexpected())),
            }
        }

        let op = match op.unwrap_or_default() {
            OperationName::Suite => Operation::Suite,
        };
        Ok((Options { op }, vec![]))
    }
}

#[allow(dead_code)]
pub fn run(_options: Options, _ctx: impl terminal::Context) -> anyhow::Result<()> {
    Ok(())
}
