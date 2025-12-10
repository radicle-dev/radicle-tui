#[path = "inbox/common.rs"]
mod common;
#[path = "inbox/list.rs"]
mod list;

use std::ffi::OsString;

use anyhow::anyhow;

use radicle::storage::{HasRepoId, ReadRepository};

use radicle_cli::terminal;
use radicle_cli::terminal::{Args, Error, Help};

use self::common::{Mode, RepositoryMode, SelectionMode};

use crate::ui::items::notification::filter::{NotificationFilter, SortBy};

pub const HELP: Help = Help {
    name: "inbox",
    description: "Terminal interfaces for notifications",
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad-tui inbox list [<option>...]

List options

    --mode <MODE>           Set selection mode; see MODE below (default: operation)
    --json                  Return JSON on stderr instead of calling `rad`

    --sort-by <field>       Sort by `id` or `timestamp` (default: timestamp)
    --reverse, -r           Reverse the list

    The MODE argument can be 'operation' or 'id'. 'operation' selects a notification id and
    an operation, whereas 'id' selects a notification id only.

Other options

    --no-forward            Don't forward command to `rad` (default: true)
    --help                  Print help (enables forwarding)
"#,
};

#[derive(Debug, PartialEq)]
pub struct Options {
    op: Operation,
}

#[derive(Debug, PartialEq)]
pub enum Operation {
    List { opts: ListOptions },
    Other { args: Vec<OsString> },
    Unknown { args: Vec<OsString> },
}

#[derive(PartialEq, Eq)]
pub enum OperationName {
    List,
    Unknown,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ListOptions {
    mode: Mode,
    filter: NotificationFilter,
    sort_by: SortBy,
    json: bool,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args.clone());
        let mut op = OperationName::List;
        let mut forward = None;
        let mut json = false;
        let mut help = false;
        let mut repository_mode = None;
        let mut reverse = None;
        let mut field = None;
        let mut list_opts = ListOptions::default();

        while let Some(arg) = parser.next()? {
            match arg {
                Long("no-forward") => {
                    forward = Some(false);
                }
                Long("json") => {
                    json = true;
                }
                Long("help") | Short('h') => {
                    help = true;
                    // Only enable forwarding if it was not already disabled explicitly
                    forward = match forward {
                        Some(false) => Some(false),
                        _ => Some(true),
                    };
                }

                // list options.
                Long("mode") | Short('m') if op == OperationName::List => {
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

                Long("repo") if repository_mode.is_none() => {
                    let val = parser.value()?;
                    let repo = terminal::args::rid(&val)?;

                    repository_mode = Some(RepositoryMode::ByRepo((repo, None)));
                }
                Long("all") | Short('a') if repository_mode.is_none() => {
                    repository_mode = Some(RepositoryMode::All);
                }

                Value(val) if op == OperationName::List => match val.to_string_lossy().as_ref() {
                    "list" => op = OperationName::List,
                    _ => {
                        op = OperationName::Unknown;
                        // Only enable forwarding if it was not already disabled explicitly
                        forward = match forward {
                            Some(false) => Some(false),
                            _ => Some(true),
                        };
                    }
                },
                _ => {
                    if op == OperationName::List {
                        return Err(anyhow!(arg.unexpected()));
                    }
                }
            }
        }

        // Disable forwarding if it was not enabled via `--help` or was
        // not disabled explicitly.
        let forward = forward.unwrap_or_default();

        // Show local help
        if help && !forward {
            return Err(Error::Help.into());
        }

        list_opts.mode = list_opts
            .mode
            .with_repository(repository_mode.unwrap_or_default());
        list_opts.sort_by = if let Some(field) = field {
            SortBy {
                field,
                reverse: reverse.unwrap_or(false),
            }
        } else {
            SortBy::default()
        };

        // Map local commands. Forward help and ignore `no-forward`.
        let op = match op {
            OperationName::List if !forward => Operation::List {
                opts: ListOptions { json, ..list_opts },
            },
            OperationName::Unknown if !forward => Operation::Unknown { args },
            _ => Operation::Other { args },
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
            let repository = profile.storage.repository(rid)?;

            if let Err(err) = crate::log::enable() {
                println!("{err}");
            }
            log::info!("Starting inbox listing interface in project {rid}..");

            let context = list::Context {
                profile,
                project: repository.identity_doc()?.project()?,
                rid: repository.rid(),
                mode: opts.mode,
                filter: opts.filter.clone(),
                sort_by: opts.sort_by,
            };
            let selection = list::Tui::new(context).run().await?;

            if opts.json {
                let selection = selection
                    .map(|o| serde_json::to_string(&o).unwrap_or_default())
                    .unwrap_or_default();

                log::info!("About to print to `stderr`: {selection}");
                log::info!("Exiting inbox listing interface..");

                eprint!("{selection}");
            } else if let Some(selection) = selection {
                let mut args = vec![];

                if let Some(operation) = selection.operation {
                    args.push(operation.to_string());
                }
                if let Some(id) = selection.ids.first() {
                    args.push(format!("{id}"));
                }

                let args = args.into_iter().map(OsString::from).collect::<Vec<_>>();
                let _ = crate::terminal::run_rad(Some("inbox"), &args);
            }
        }
        Operation::Other { args } => {
            let _ = crate::terminal::run_rad(Some("inbox"), &args);
        }
        Operation::Unknown { .. } => {
            anyhow::bail!("unknown operation provided");
        }
    }

    Ok(())
}

#[cfg(test)]
mod cli {
    use radicle_cli::terminal::args::Error;
    use radicle_cli::terminal::Args;

    use super::{ListOptions, Operation, Options};

    #[test]
    fn empty_operation_should_default_to_list_and_not_be_forwarded(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let expected_op = Operation::List {
            opts: ListOptions::default(),
        };

        let args = vec![];
        let (actual, _) = Options::from_args(args)?;
        assert_eq!(actual.op, expected_op);

        Ok(())
    }

    #[test]
    fn empty_operation_with_help_should_be_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let args = vec!["--help".into()];
        let expected_op = Operation::Other { args: args.clone() };

        let (actual, _) = Options::from_args(args)?;
        assert_eq!(actual.op, expected_op);

        Ok(())
    }

    #[test]
    fn empty_operation_with_help_should_not_be_forwarded() -> Result<(), Box<dyn std::error::Error>>
    {
        let args = vec!["--help".into(), "--no-forward".into()];

        let actual = Options::from_args(args).unwrap_err().downcast::<Error>()?;
        assert!(matches!(actual, Error::Help));

        Ok(())
    }

    #[test]
    fn empty_operation_should_not_be_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let expected_op = Operation::List {
            opts: ListOptions::default(),
        };

        let args = vec!["--no-forward".into()];
        let (actual, _) = Options::from_args(args)?;
        assert_eq!(actual.op, expected_op);

        Ok(())
    }

    #[test]
    fn list_operation_should_not_be_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let expected_op = Operation::List {
            opts: ListOptions::default(),
        };

        let args = vec!["list".into()];
        let (actual, _) = Options::from_args(args)?;
        assert_eq!(actual.op, expected_op);

        Ok(())
    }

    #[test]
    fn list_operation_should_not_be_forwarded_explicitly() -> Result<(), Box<dyn std::error::Error>>
    {
        let expected_op = Operation::List {
            opts: ListOptions::default(),
        };

        let args = vec!["list".into(), "--no-forward".into()];
        let (actual, _) = Options::from_args(args)?;
        assert_eq!(actual.op, expected_op);

        Ok(())
    }

    #[test]
    fn list_operation_with_help_should_be_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let args = vec!["list".into(), "--help".into()];
        let expected_op = Operation::Other { args: args.clone() };

        let (actual, _) = Options::from_args(args)?;
        assert_eq!(actual.op, expected_op);

        Ok(())
    }

    #[test]
    fn list_operation_with_help_should_not_be_forwarded() -> Result<(), Box<dyn std::error::Error>>
    {
        let args = vec!["list".into(), "--help".into(), "--no-forward".into()];
        let actual = Options::from_args(args).unwrap_err().downcast::<Error>()?;

        assert!(matches!(actual, Error::Help));

        Ok(())
    }

    #[test]
    fn list_operation_with_help_should_not_be_forwarded_reversed(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let args = vec!["list".into(), "--no-forward".into(), "--help".into()];
        let actual = Options::from_args(args).unwrap_err().downcast::<Error>()?;

        assert!(matches!(actual, Error::Help));

        Ok(())
    }

    #[test]
    fn unknown_operation_should_be_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let args = vec!["operation".into()];
        let expected_op = Operation::Other { args: args.clone() };

        let (actual, _) = Options::from_args(args)?;
        assert_eq!(actual.op, expected_op);

        Ok(())
    }

    #[test]
    fn unknown_operation_should_not_be_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let args = vec!["operation".into(), "--no-forward".into()];
        let expected_op = Operation::Unknown { args: args.clone() };

        let (actual, _) = Options::from_args(args)?;
        assert_eq!(actual.op, expected_op);

        Ok(())
    }
}
