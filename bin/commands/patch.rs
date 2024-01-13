#[path = "patch/common.rs"]
mod common;
#[path = "patch/list.rs"]
mod list;
#[path = "patch/select.rs"]
mod select;
#[path = "patch/suite.rs"]
mod suite;

use std::ffi::OsString;
use std::process::Command;

use anyhow::anyhow;

use radicle_tui::{context, log, Window};

use crate::terminal;
use crate::terminal::args::{Args, Error, Help};

use self::list::PatchCommand;

pub const FPS: u64 = 60;
pub const HELP: Help = Help {
    name: "patch",
    description: "Terminal interfaces for patches",
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad-tui patch select
    rad-tui patch list

General options

    --help               Print help
"#,
};

pub struct Options {
    op: Operation,
}

pub enum Operation {
    List,
    Select,
}

#[derive(PartialEq, Eq)]
pub enum OperationName {
    List,
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
                    "list" => op = Some(OperationName::List),
                    "select" => op = Some(OperationName::Select),
                    unknown => anyhow::bail!("unknown operation '{}'", unknown),
                },
                _ => return Err(anyhow!(arg.unexpected())),
            }
        }

        let op = match op.ok_or_else(|| anyhow!("an operation must be provided"))? {
            OperationName::List => Operation::List,
            OperationName::Select => Operation::Select,
        };
        Ok((Options { op }, vec![]))
    }
}

pub fn run(options: Options, _ctx: impl terminal::Context) -> anyhow::Result<()> {
    let (_, id) = radicle::rad::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;

    match options.op {
        Operation::List => {
            let context = context::Context::new(id)?.with_patches();

            log::enable(context.profile(), "patch", "list")?;

            let mut app = list::App::new(context);
            if let Some(command) = Window::default().run(&mut app, 1000 / FPS)? {
                match command {
                    PatchCommand::Show(id) => {
                        match Command::new("rad")
                            .arg("patch")
                            .arg("show")
                            .arg(id.to_string())
                            .spawn()
                        {
                            Ok(_) => {}
                            Err(_) => {}
                        }
                    },
                    PatchCommand::Edit(id) => {
                        match Command::new("rad")
                            .arg("patch")
                            .arg("edit")
                            .arg(id.to_string())
                            .spawn()
                        {
                            Ok(_) => {}
                            Err(_) => {}
                        }
                    }
                    PatchCommand::Checkout(id) => {
                        match Command::new("rad")
                            .arg("patch")
                            .arg("checkout")
                            .arg(id.to_string())
                            .spawn()
                        {
                            Ok(_) => {}
                            Err(_) => {}
                        }
                    }
                }
            }

            // eprint!("{patch_id}");
        }
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
