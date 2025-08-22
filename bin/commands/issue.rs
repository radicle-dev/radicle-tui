#[path = "issue/common.rs"]
mod common;
#[path = "issue/list.rs"]
mod list;

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

    rad-tui issue list [<option>...]

List options

    --mode <MODE>       Set selection mode; see MODE below (default: operation)
    --json              Return JSON on stderr instead of calling `rad`

    The MODE argument can be 'operation' or 'id'. 'operation' selects an issue id and
    an operation, whereas 'id' selects an issue id only.

Other options

    --no-forward        Don't forward command to `rad` (default: true)
    --help              Print help (enables forwarding)
"#,
};

#[derive(Debug, PartialEq)]
pub struct Options {
    op: Operation,
    repo: Option<RepoId>,
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

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ListOptions {
    mode: common::Mode,
    filter: cob::issue::Filter,
    json: bool,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args.clone());
        let mut op = OperationName::List;
        let mut repo = None;
        let mut forward = None;
        let mut json = false;
        let mut help = false;
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

                // select options.
                Long("mode") | Short('m') if op == OperationName::List => {
                    let val = parser.value()?;
                    let val = val.to_str().unwrap_or_default();

                    list_opts.mode = match val {
                        "operation" => common::Mode::Operation,
                        "id" => common::Mode::Id,
                        unknown => anyhow::bail!("unknown mode '{}'", unknown),
                    };
                }
                Long("all") if op == OperationName::List => {
                    list_opts.filter = list_opts.filter.with_state(None);
                }
                Long("open") if op == OperationName::List => {
                    list_opts.filter = list_opts.filter.with_state(Some(issue::State::Open));
                }
                Long("solved") if op == OperationName::List => {
                    list_opts.filter = list_opts.filter.with_state(Some(issue::State::Closed {
                        reason: issue::CloseReason::Solved,
                    }));
                }
                Long("closed") if op == OperationName::List => {
                    list_opts.filter = list_opts.filter.with_state(Some(issue::State::Closed {
                        reason: issue::CloseReason::Other,
                    }));
                }
                Long("assigned") if op == OperationName::List => {
                    if let Ok(val) = parser.value() {
                        list_opts.filter =
                            list_opts.filter.with_assginee(terminal::args::did(&val)?);
                    } else {
                        list_opts.filter = list_opts.filter.with_assgined(true);
                    }
                }

                Long("repo") => {
                    let val = parser.value()?;
                    let rid = terminal::args::rid(&val)?;

                    repo = Some(rid);
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

        // Map local commands. Forward help and ignore `no-forward`.
        let op = match op {
            OperationName::List if !forward => Operation::List {
                opts: ListOptions { json, ..list_opts },
            },
            OperationName::Unknown if !forward => Operation::Unknown { args },
            _ => Operation::Other { args },
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
        Operation::List { opts } => {
            let profile = ctx.profile()?;
            let rid = options.repo.unwrap_or(rid);
            let repository = profile.storage.repository(rid).unwrap();

            if let Err(err) = crate::log::enable() {
                println!("{}", err);
            }
            log::info!("Starting issue listing interface in project {}..", rid);

            let context = list::Context {
                profile,
                repository,
                mode: opts.mode,
                filter: opts.filter.clone(),
            };

            let selection = list::App::new(context, terminal_info).run().await?;

            if opts.json {
                let selection = selection
                    .map(|o| serde_json::to_string(&o).unwrap_or_default())
                    .unwrap_or_default();

                log::info!("About to print to `stderr`: {}", selection);
                log::info!("Exiting issue listing interface..");

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
                let _ = crate::terminal::run_rad(Some("issue"), &args);
            }
        }
        Operation::Other { args } => {
            let _ = crate::terminal::run_rad(Some("issue"), &args);
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
