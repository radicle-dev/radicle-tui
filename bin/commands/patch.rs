#[path = "patch/select.rs"]
mod select;
#[path = "patch/suite.rs"]
mod suite;

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
    Select,
}

#[derive(PartialEq, Eq)]
pub enum OperationName {
    Select,
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
                    "select" => op = Some(OperationName::Select),
                    unknown => anyhow::bail!("unknown operation '{}'", unknown),
                },
                _ => return Err(anyhow!(arg.unexpected())),
            }
        }

        let op = match op.ok_or_else(|| anyhow!("an operation must be provided"))? {
            OperationName::Select => Operation::Select,
        };
        Ok((Options { op }, vec![]))
    }
}

pub fn run(options: Options, _ctx: impl terminal::Context) -> anyhow::Result<()> {
    let (_, id) = radicle::rad::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;

    match options.op {
        Operation::Select => {
            let context = context::Context::new(id)?.with_patches();

            log::enable(context.profile(), "patch", "select")?;

            let patch_id = Window::default()
                .run(&mut select::App::new(context), 1000 / FPS)?
                .unwrap_or_default();

            eprint!("{patch_id}");
        }
    }

    Ok(())
}
