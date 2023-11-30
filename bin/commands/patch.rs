#[path = "patch/suite.rs"]
mod suite;
#[path = "patch/list.rs"]
mod list;

use std::ffi::OsString;

use anyhow::anyhow;

use radicle_tui::{context, log, Window};

use crate::terminal;
use crate::terminal::args::{Args, Error, Help};

pub const FPS: u64 = 60;
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

pub struct Options {
    op: Operation,
}

pub enum Operation {
    List,
}

#[derive(PartialEq, Eq)]
pub enum OperationName {
    List,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut op: Option<OperationName> = None;

        #[allow(clippy::never_loop)]
        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") | Short('h') => {
                    return Err(Error::Help.into());
                }
                Value(val) if op.is_none() => match val.to_string_lossy().as_ref() {
                    "list" => op = Some(OperationName::List),
                    unknown => anyhow::bail!("unknown operation '{}'", unknown),
                },
                _ => return Err(anyhow!(arg.unexpected())),
            }
        }

        let op = match op.ok_or_else(|| anyhow!("an operation must be provided"))? {
            OperationName::List => Operation::List,
        };
        Ok((Options { op }, vec![]))
    }
}

pub fn run(options: Options, _ctx: impl terminal::Context) -> anyhow::Result<()> {
    let (_, id) = radicle::rad::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;

    let context = context::Context::new(id)?;

    match options.op {
        Operation::List => {
            log::enable("patch", "list", context.profile())?;

            let patch_id = Window::default()
                .run(&mut list::App::new(context), 1000 / FPS)?
                .ok_or_else(|| anyhow!("expected patch id"))?;

            eprint!("{patch_id}");
        }
    }

    Ok(())
}
