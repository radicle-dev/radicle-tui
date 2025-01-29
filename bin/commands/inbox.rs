#[path = "inbox/common.rs"]
mod common;
#[path = "inbox/list.rs"]
mod list;

use std::ffi::OsString;

use anyhow::anyhow;

use radicle_cli::terminal;
use radicle_cli::terminal::{Args, Error, Help};

use self::common::{Mode, RepositoryMode, SelectionMode};

use crate::cob::inbox;

pub const HELP: Help = Help {
    name: "inbox",
    description: "Terminal interfaces for notifications",
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad-tui inbox list [<option>...]

List options

    --mode <MODE>           Set selection mode; see MODE below (default: operation)
    
    --sort-by <field>       Sort by `id` or `timestamp` (default: timestamp)
    --reverse, -r           Reverse the list

    The MODE argument can be 'operation' or 'id'. 'operation' selects a notification id and
    an operation, whereas 'id' selects a notification id only.

Other options

    --help                  Print help    
"#,
};

pub struct Options {
    op: Operation,
}

pub enum Operation {
    List { opts: ListOptions },
}

#[derive(PartialEq, Eq)]
pub enum OperationName {
    List,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ListOptions {
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
        let mut list_opts = ListOptions::default();

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") | Short('h') => {
                    return Err(Error::Help.into());
                }

                // select options.
                Long("mode") | Short('m') if op == Some(OperationName::List) => {
                    let val = parser.value()?;
                    let val = val.to_str().unwrap_or_default();

                    let selection_mode = match val {
                        "operation" => SelectionMode::Operation,
                        "id" => SelectionMode::Id,
                        unknown => anyhow::bail!("unknown mode '{}'", unknown),
                    };
                    list_opts.mode = list_opts.mode.with_selection(selection_mode)
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
                    "list" => op = Some(OperationName::List),
                    unknown => anyhow::bail!("unknown operation '{}'", unknown),
                },
                _ => return Err(anyhow!(arg.unexpected())),
            }
        }

        list_opts.mode = list_opts
            .mode
            .with_repository(repository_mode.unwrap_or_default());
        list_opts.sort_by = if let Some(field) = field {
            inbox::SortBy {
                field,
                reverse: reverse.unwrap_or(false),
            }
        } else {
            inbox::SortBy::default()
        };

        let op = match op.ok_or_else(|| anyhow!("an operation must be provided"))? {
            OperationName::List => Operation::List { opts: list_opts },
        };
        Ok((Options { op }, vec![]))
    }
}

#[tokio::main]
pub async fn run(options: Options, ctx: impl terminal::Context) -> anyhow::Result<()> {
    use radicle::storage::ReadStorage;

    let (_, rid) = radicle::rad::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;

    match options.op {
        Operation::List { opts } => {
            let profile = ctx.profile()?;
            let repository = profile.storage.repository(rid).unwrap();

            if let Err(err) = crate::log::enable() {
                println!("{}", err);
            }
            log::info!("Starting inbox listing interface in project {}..", rid);

            let context = list::Context {
                profile,
                repository,
                mode: opts.mode,
                filter: opts.filter.clone(),
                sort_by: opts.sort_by,
            };
            let output = list::App::new(context).run().await?;

            let output = output
                .map(|o| serde_json::to_string(&o).unwrap_or_default())
                .unwrap_or_default();

            log::info!("About to print to `stderr`: {}", output);
            log::info!("Exiting inbox listing interface..");

            eprint!("{output}");
        }
    }

    Ok(())
}
