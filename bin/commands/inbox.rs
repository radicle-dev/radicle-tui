#[path = "inbox/common.rs"]
mod common;
#[path = "inbox/select.rs"]
mod select;

use std::ffi::OsString;

use anyhow::anyhow;

use radicle_tui as tui;

use tui::common::cob::inbox::{self};

use crate::terminal;
use crate::terminal::args::{Args, Error, Help};

use self::common::{Mode, RepositoryMode, SelectionMode};

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
    mode: Mode,
    filter: inbox::Filter,
    sort_by: inbox::SortBy,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut op: Option<OperationName> = None;
        let mut repository_mode = None;
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

                    let selection_mode = match val {
                        "operation" => SelectionMode::Operation,
                        "id" => SelectionMode::Id,
                        unknown => anyhow::bail!("unknown mode '{}'", unknown),
                    };
                    select_opts.mode = select_opts.mode.with_selection(selection_mode)
                }

                Long("reverse") | Short('r') => {
                    reverse = Some(true);
                }
                Long("sort-by") => {
                    let val = parser.value()?;

                    match terminal::args::string(&val).as_str() {
                        "timestamp" => field = Some("timestamp"),
                        "id" => field = Some("id"),
                        other => anyhow::bail!("unknown sorting field '{other}'"),
                    }
                }

                Long("repo") if repository_mode.is_none() && op.is_some() => {
                    let val = parser.value()?;
                    let repo = terminal::args::rid(&val)?;

                    repository_mode = Some(RepositoryMode::ByRepo((repo, None)));
                }
                Long("all") | Short('a') if repository_mode.is_none() => {
                    repository_mode = Some(RepositoryMode::All);
                }

                Value(val) if op.is_none() => match val.to_string_lossy().as_ref() {
                    "select" => op = Some(OperationName::Select),
                    unknown => anyhow::bail!("unknown operation '{}'", unknown),
                },
                _ => return Err(anyhow!(arg.unexpected())),
            }
        }

        select_opts.mode = select_opts
            .mode
            .with_repository(repository_mode.unwrap_or_default());
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

#[tokio::main]
pub async fn run(options: Options, _ctx: impl terminal::Context) -> anyhow::Result<()> {
    use radicle::storage::ReadStorage;
    use tui::common::log;

    let (_, rid) = radicle::rad::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;

    match options.op {
        Operation::Select { opts } => {
            let profile = terminal::profile()?;
            let repository = profile.storage.repository(rid).unwrap();

            log::enable(&profile, "inbox", "select")?;

            let context = select::Context {
                profile,
                repository,
                mode: opts.mode,
                filter: opts.filter.clone(),
                sort_by: opts.sort_by,
            };
            let output = select::App::new(context).run().await?;

            let output = output
                .map(|o| serde_json::to_string(&o).unwrap_or_default())
                .unwrap_or_default();

            eprint!("{output}");
        }
    }

    Ok(())
}
