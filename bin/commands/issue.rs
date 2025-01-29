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

pub struct Options {
    op: Operation,
    repo: Option<RepoId>,
}

pub enum Operation {
    List { opts: ListOptions },
    Other { args: Vec<OsString> },
}

#[derive(PartialEq, Eq)]
pub enum OperationName {
    List,
    Other,
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
                    _ => op = OperationName::Other,
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
                let _ = crate::terminal::run_rad("issue", &args);
            }
        }
        Operation::Other { args } => {
            let _ = crate::terminal::run_rad("issue", &args);
        }
    }

    Ok(())
}

#[cfg(test)]
mod cli {
    use std::process::Command;

    use assert_cmd::prelude::*;

    use predicates::prelude::*;

    mod assert {
        use predicates::prelude::*;
        use predicates::str::ContainsPredicate;

        pub fn is_tui() -> ContainsPredicate {
            predicate::str::contains("Inappropriate ioctl for device")
        }

        pub fn is_rad_manual() -> ContainsPredicate {
            predicate::str::contains("rad-issue")
        }

        pub fn is_issue_help() -> ContainsPredicate {
            predicate::str::contains("Terminal interfaces for issues")
        }
    }

    #[test]
    #[ignore = "breaks stdout"]
    fn empty_operation() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.arg("issue");
        cmd.assert().failure().stdout(assert::is_tui());

        Ok(())
    }

    #[test]
    #[ignore = "breaks stdout"]
    fn empty_operation_is_not_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.arg("issue");
        cmd.assert().failure().stdout(assert::is_tui());

        Ok(())
    }

    #[test]
    #[ignore = "breaks stdout"]
    fn empty_operation_with_help_is_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.args(["issue", "--help"]);
        cmd.assert().success().stdout(assert::is_rad_manual());

        Ok(())
    }

    #[test]
    #[ignore = "breaks stdout"]
    fn empty_operation_with_help_is_not_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.args(["issue", "--help", "--no-forward"]);
        cmd.assert().success().stdout(assert::is_issue_help());

        Ok(())
    }

    #[test]
    #[ignore = "breaks stdout"]
    fn empty_operation_is_not_forwarded_explicitly() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.args(["issue", "--no-forward"]);
        cmd.assert().failure().stdout(assert::is_tui());

        Ok(())
    }

    #[test]
    #[ignore = "breaks stdout"]
    fn list_operation_is_not_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.args(["issue", "list"]);
        cmd.assert().failure().stdout(assert::is_tui());

        Ok(())
    }

    #[test]
    #[ignore = "breaks stdout"]
    fn list_operation_is_not_forwarded_explicitly() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.args(["issue", "list", "--no-forward"]);
        cmd.assert().failure().stdout(assert::is_tui());

        Ok(())
    }

    #[test]
    #[ignore = "breaks stdout"]
    fn list_operation_with_help_is_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.args(["issue", "list", "--help"]);
        cmd.assert().success().stdout(assert::is_rad_manual());

        Ok(())
    }

    #[test]
    #[ignore = "breaks stdout"]
    fn list_operation_with_help_is_not_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.args(["issue", "list", "--help", "--no-forward"]);
        cmd.assert().success().stdout(assert::is_issue_help());

        Ok(())
    }

    #[test]
    #[ignore = "breaks stdout"]
    fn list_operation_with_help_is_not_forwarded_reversed() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.args(["issue", "list", "--no-forward", "--help"]);
        cmd.assert().success().stdout(assert::is_issue_help());

        Ok(())
    }

    #[test]
    #[ignore = "breaks stdout"]
    fn unknown_operation_show_is_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.args(["issue", "show"]);
        cmd.assert().success().stdout(predicate::str::contains(
            "Error: rad issue: an issue must be provided",
        ));

        Ok(())
    }

    #[test]
    #[ignore = "breaks stdout"]
    fn unknown_operation_edit_is_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.args(["issue", "edit"]);
        cmd.assert().success().stdout(predicate::str::contains(
            "Error: rad issue: an issue must be provided",
        ));

        Ok(())
    }

    #[test]
    #[ignore = "breaks stdout"]
    fn unknown_operation_is_not_forwarded() -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::cargo_bin("rad-tui")?;

        cmd.args(["issue", "operation", "--no-forward"]);
        cmd.assert().success().stdout(predicate::str::contains(
            "Error: rad issue: unknown operation",
        ));

        Ok(())
    }
}
