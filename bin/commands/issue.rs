#[path = "issue/common.rs"]
mod common;
#[path = "issue/select.rs"]
mod select;
#[path = "issue/suite.rs"]
mod suite;

use std::ffi::OsString;

use anyhow::anyhow;

use radicle_tui as tui;

use tui::cob::issue::{self, State};
use tui::{context, log, Window};

use crate::terminal;
use crate::terminal::args::{Args, Error, Help};

pub const FPS: u64 = 60;
pub const HELP: Help = Help {
    name: "issue",
    description: "Terminal interfaces for issues",
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad-tui patch select [<option>...]

Select options

    --mode <MODE>           Set selection mode; see MODE below (default: operation)

    The MODE argument can be 'operation' or 'id'. 'operation' selects an issue id and
    an operation, whereas 'id' selects an issue id only.

Other options

    --help               Print help
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

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SelectOptions {
    mode: select::Mode,
    filter: issue::Filter,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut op: Option<OperationName> = None;
        let mut json = false;
        let mut select_opts = SelectOptions::default();

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") | Short('h') => {
                    return Err(Error::Help.into());
                }
                Long("json") | Short('j') => {
                    json = true;
                }

                // select options.
                Long("mode") | Short('m') if op == Some(OperationName::Select) => {
                    let val = parser.value()?;
                    let val = val.to_str().unwrap_or_default();

                    select_opts.mode = match val {
                        "operation" => select::Mode::Operation,
                        "id" => select::Mode::Id,
                        unknown => anyhow::bail!("unknown mode '{}'", unknown),
                    };
                }
                Long("all") if op == Some(OperationName::Select) => {
                    select_opts.filter = select_opts.filter.with_state(None);
                }
                Long("open") if op == Some(OperationName::Select) => {
                    select_opts.filter = select_opts.filter.with_state(Some(State::Open));
                }
                Long("solved") if op == Some(OperationName::Select) => {
                    select_opts.filter = select_opts.filter.with_state(Some(State::Solved));
                }
                Long("closed") if op == Some(OperationName::Select) => {
                    select_opts.filter = select_opts.filter.with_state(Some(State::Closed));
                }
                Long("assigned") if op == Some(OperationName::Select) => {
                    if let Ok(val) = parser.value() {
                        select_opts.filter =
                            select_opts.filter.with_assginee(terminal::args::did(&val)?);
                    } else {
                        select_opts.filter = select_opts.filter.with_assgined(true);
                    }
                }

                Value(val) if op.is_none() => match val.to_string_lossy().as_ref() {
                    "select" => op = Some(OperationName::Select),
                    unknown => anyhow::bail!("unknown operation '{}'", unknown),
                },
                _ => return Err(anyhow!(arg.unexpected())),
            }
        }

        let op = match op.ok_or_else(|| anyhow!("an operation must be provided"))? {
            OperationName::Select => Operation::Select { opts: select_opts },
        };
        Ok((Options { op, json }, vec![]))
    }
}

pub fn run(options: Options, _ctx: impl terminal::Context) -> anyhow::Result<()> {
    let (_, id) = radicle::rad::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;

    match options.op {
        Operation::Select { ref opts } => {
            let profile = terminal::profile()?;
            let context = context::Context::new(profile, id)?.with_issues();

            log::enable(context.profile(), "issue", "select")?;

            let mut app = select::App::new(context, opts.mode.clone(), opts.filter.clone());
            let output = Window::default().run(&mut app, 1000 / FPS)?;

            let output = if options.json {
                output
                    .map(|o| serde_json::to_string(&o).unwrap_or_default())
                    .unwrap_or_default()
            } else {
                match options.op {
                    Operation::Select { ref opts } => match &opts.mode {
                        select::Mode::Id => output.map(|o| format!("{}", o)).unwrap_or_default(),
                        select::Mode::Operation => output
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
