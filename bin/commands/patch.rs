#[path = "patch/common.rs"]
mod common;
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

    rad-tui patch [<option>...]
    rad-tui patch select [--operation | --id] [<option>...]

Select options

    --operation         Select patch id and operation (default)
    --id                Select patch id only
    


Other options

    --json              Output is JSON (default: false)
    --help              Print help
"#,
};

pub struct Options {
    op: Operation,
    json: bool,
}

pub enum Operation {
    Select { opts: SelectOptions },
}

#[derive(PartialEq, Eq)]
pub enum OperationName {
    Select,
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct SelectOptions {
    subject: select::Subject,
}

impl SelectOptions {
    pub fn new(subject: select::Subject) -> Self {
        Self { subject }
    }
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut op: Option<OperationName> = None;
        let mut json = false;
        let mut select_opts: Option<SelectOptions> = None;

        #[allow(clippy::never_loop)]
        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") | Short('h') => {
                    return Err(Error::Help.into());
                }
                Long("json") | Short('j') => {
                    json = true;
                }

                // Select options.
                Long("operation") | Short('o') if op == Some(OperationName::Select) => {
                    if select_opts.is_some() {
                        anyhow::bail!("select option already given")
                    }
                    select_opts = Some(SelectOptions::new(select::Subject::Operation));
                }
                Long("id") | Short('i') if op == Some(OperationName::Select) => {
                    if select_opts.is_some() {
                        anyhow::bail!("select option already given")
                    }
                    select_opts = Some(SelectOptions::new(select::Subject::Id));
                }

                Value(val) if op.is_none() => match val.to_string_lossy().as_ref() {
                    "select" => op = Some(OperationName::Select),
                    unknown => anyhow::bail!("unknown operation '{}'", unknown),
                },
                _ => return Err(anyhow!(arg.unexpected())),
            }
        }

        let op = match op.ok_or_else(|| anyhow!("an operation must be provided"))? {
            OperationName::Select => Operation::Select {
                opts: select_opts.unwrap_or_default(),
            },
        };
        Ok((Options { op, json }, vec![]))
    }
}

pub fn run(options: Options, _ctx: impl terminal::Context) -> anyhow::Result<()> {
    let (_, id) = radicle::rad::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;

    match options.op {
        Operation::Select { opts } => {
            let profile = terminal::profile()?;
            let context = context::Context::new(profile, id)?.with_patches();

            log::enable(context.profile(), "patch", "select")?;

            let mut app = select::App::new(context, opts.subject);
            let output = Window::default().run(&mut app, 1000 / FPS)?;

            let output = if options.json {
                output
                    .map(|o| serde_json::to_string(&o).unwrap_or_default())
                    .unwrap_or_default()
            } else {
                match options.op {
                    Operation::Select { opts } => match opts.subject {
                        select::Subject::Id => output.map(|o| format!("{}", o)).unwrap_or_default(),
                        select::Subject::Operation => output
                            .map(|o| format!("rad patch {}", o))
                            .unwrap_or_default(),
                    },
                }
            };

            eprint!("{output}");
        }
    }

    Ok(())
}
