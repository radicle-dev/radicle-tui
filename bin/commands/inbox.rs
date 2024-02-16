#[path = "inbox/select.rs"]
mod select;

use std::ffi::OsString;

use anyhow::anyhow;

use radicle_tui as tui;

use tui::cob::inbox::{self};
use tui::{context, log, Window};

use crate::terminal;
use crate::terminal::args::{Args, Error, Help};

pub const FPS: u64 = 60;
pub const HELP: Help = Help {
    name: "inbox",
    description: "Terminal interfaces for notifications",
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad-tui inbox select [<option>...]

Other options

    --mode <MODE>           Set selection mode; see MODE below (default: operation)
    
    --sort-by <field>       Sort by `id` or `timestamp` (default: timestamp)
    --reverse, -r           Reverse the list
    --help                  Print help

    The MODE argument can be 'operation' or 'id'. 'operation' selects a notification id and
    an operation, whereas 'id' selects a notification id only.
"#,
};

pub struct Options {
    op: Operation,
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
    filter: inbox::Filter,
    sort_by: inbox::SortBy,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut op: Option<OperationName> = None;
        let mut reverse = None;
        let mut field = None;
        let mut select_opts = SelectOptions::default();

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") | Short('h') => {
                    return Err(Error::Help.into());
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

                Long("reverse") | Short('r') => {
                    reverse = Some(true);
                }
                Long("sort-by") => {
                    let val = parser.value()?;

                    match terminal::args::string(&val).as_str() {
                        "timestamp" => field = Some("timestamp"),
                        "rowid" => field = Some("id"),
                        other => anyhow::bail!("unknown sorting field '{other}'"),
                    }
                }

                Value(val) if op.is_none() => match val.to_string_lossy().as_ref() {
                    "select" => op = Some(OperationName::Select),
                    unknown => anyhow::bail!("unknown operation '{}'", unknown),
                },
                _ => return Err(anyhow!(arg.unexpected())),
            }
        }

        select_opts.sort_by = if let Some(field) = field {
            inbox::SortBy {
                field,
                reverse: reverse.unwrap_or(false),
            }
        } else {
            inbox::SortBy::default()
        };

        let op = match op.ok_or_else(|| anyhow!("an operation must be provided"))? {
            OperationName::Select => Operation::Select { opts: select_opts },
        };
        Ok((Options { op }, vec![]))
    }
}

pub fn run(options: Options, _ctx: impl terminal::Context) -> anyhow::Result<()> {
    let (_, id) = radicle::rad::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;

    match options.op {
        Operation::Select { opts } => {
            let profile = terminal::profile()?;
            let context = context::Context::new(profile, id)?;

            log::enable(context.profile(), "inbox", "select")?;

            let mut app = select::App::new(
                context,
                opts.mode.clone(),
                opts.filter.clone(),
                opts.sort_by,
            );
            let output = Window::default().run(&mut app, 1000 / FPS)?;

            let output = output
                .map(|o| serde_json::to_string(&o).unwrap_or_default())
                .unwrap_or_default();

            eprint!("{output}");
        }
    }

    Ok(())
}