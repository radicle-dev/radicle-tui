#[path = "issue/common.rs"]
mod common;
#[path = "issue/select.rs"]
mod select;

use std::ffi::OsString;

use anyhow::anyhow;

use lazy_static::lazy_static;

use radicle::identity::RepoId;
use radicle::issue;

use radicle_cli::terminal;
use radicle_cli::terminal::{Args, Error, Help};

use crate::cob;
use crate::ui::TerminalInfo;

lazy_static! {
    static ref TERMINAL_INFO: TerminalInfo = TerminalInfo {
        luma: Some(terminal_light::luma().unwrap_or_default())
    };
}

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
    repo: Option<RepoId>,
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
    mode: common::Mode,
    filter: cob::issue::Filter,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut op: Option<OperationName> = None;
        let mut repo = None;
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
                        "operation" => common::Mode::Operation,
                        "id" => common::Mode::Id,
                        unknown => anyhow::bail!("unknown mode '{}'", unknown),
                    };
                }
                Long("all") if op == Some(OperationName::Select) => {
                    select_opts.filter = select_opts.filter.with_state(None);
                }
                Long("open") if op == Some(OperationName::Select) => {
                    select_opts.filter = select_opts.filter.with_state(Some(issue::State::Open));
                }
                Long("solved") if op == Some(OperationName::Select) => {
                    select_opts.filter =
                        select_opts.filter.with_state(Some(issue::State::Closed {
                            reason: issue::CloseReason::Solved,
                        }));
                }
                Long("closed") if op == Some(OperationName::Select) => {
                    select_opts.filter =
                        select_opts.filter.with_state(Some(issue::State::Closed {
                            reason: issue::CloseReason::Other,
                        }));
                }
                Long("assigned") if op == Some(OperationName::Select) => {
                    if let Ok(val) = parser.value() {
                        select_opts.filter =
                            select_opts.filter.with_assginee(terminal::args::did(&val)?);
                    } else {
                        select_opts.filter = select_opts.filter.with_assgined(true);
                    }
                }

                Long("repo") => {
                    let val = parser.value()?;
                    let rid = terminal::args::rid(&val)?;

                    repo = Some(rid);
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
        Ok((Options { op, repo }, vec![]))
    }
}

#[tokio::main]
pub async fn run(options: Options, ctx: impl terminal::Context) -> anyhow::Result<()> {
    use radicle::storage::ReadStorage;

    let (_, rid) = radicle::rad::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;

    let terminal_info = TERMINAL_INFO.clone();

    match options.op {
        Operation::Select { opts } => {
            let profile = ctx.profile()?;
            let rid = options.repo.unwrap_or(rid);
            let repository = profile.storage.repository(rid).unwrap();

            if let Err(err) = crate::log::enable() {
                println!("{}", err);
            }
            log::info!("Starting issue selection interface in project {}..", rid);

            let context = select::Context {
                profile,
                repository,
                mode: opts.mode,
                filter: opts.filter.clone(),
            };

            let output = select::App::new(context, terminal_info).run().await?;

            let output = output
                .map(|o| serde_json::to_string(&o).unwrap_or_default())
                .unwrap_or_default();

            log::info!("About to print to `stderr`: {}", output);
            log::info!("Exiting issue selection interface..");

            eprint!("{output}");
        }
    }

    Ok(())
}
