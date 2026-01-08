#[path = "issue/list.rs"]
mod list;

use std::ffi::OsString;

use anyhow::anyhow;

use lazy_static::lazy_static;

use radicle::cob::thread::CommentId;
use radicle::identity::RepoId;
use radicle::issue::{IssueId, State};
use radicle::prelude::Did;
use radicle::{issue, storage, Profile};

use radicle_cli as cli;

use cli::terminal::patch::Message;
use cli::terminal::Context;
use cli::terminal::{Args, Error, Help};

use crate::commands::tui_issue::list::IssueOperation;
use crate::terminal;
use crate::ui::items::filter::DidFilter;
use crate::ui::items::issue::filter::IssueFilter;
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

    --json              Return JSON on stderr instead of calling `rad`

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListFilter {
    state: Option<State>,
    assigned: bool,
    assignees: Vec<Did>,
}

impl Default for ListFilter {
    fn default() -> Self {
        Self {
            state: Some(State::default()),
            assigned: false,
            assignees: vec![],
        }
    }
}

impl ListFilter {
    pub fn with_state(mut self, state: Option<State>) -> Self {
        self.state = state;
        self
    }

    pub fn with_assgined(mut self, assigned: bool) -> Self {
        self.assigned = assigned;
        self
    }

    pub fn with_assginee(mut self, assignee: Did) -> Self {
        self.assignees.push(assignee);
        self
    }
}

#[allow(clippy::from_over_into)]
impl Into<IssueFilter> for (Did, ListFilter) {
    fn into(self) -> IssueFilter {
        let (me, mut filter) = self;
        let mut and = filter
            .state
            .map(|s| vec![IssueFilter::State(s)])
            .unwrap_or(vec![]);

        let mut assignees = filter.assigned.then_some(vec![me]).unwrap_or_default();
        assignees.append(&mut filter.assignees);

        if assignees.len() == 1 {
            and.push(IssueFilter::Assignee(DidFilter::Single(
                *assignees.first().unwrap(),
            )));
        } else if assignees.len() > 1 {
            and.push(IssueFilter::Assignee(DidFilter::Or(assignees)));
        }

        IssueFilter::And(and)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ListOptions {
    filter: ListFilter,
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
                        list_opts.filter = list_opts
                            .filter
                            .with_assginee(cli::terminal::args::did(&val)?);
                    } else {
                        list_opts.filter = list_opts.filter.with_assgined(true);
                    }
                }

                Long("repo") => {
                    let val = parser.value()?;
                    let rid = cli::terminal::args::rid(&val)?;

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
pub async fn run(options: Options, ctx: impl Context) -> anyhow::Result<()> {
    use radicle::storage::ReadStorage;

    let (_, rid) = radicle::rad::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;

    let terminal_info = TERMINAL_INFO.clone();

    match options.op {
        Operation::List { opts } => {
            if let Err(err) = crate::log::enable() {
                println!("{err}");
            }
            log::info!("Starting issue listing interface in project {rid}..");

            #[derive(Default)]
            struct PreviousState {
                issue_id: Option<IssueId>,
                comment_id: Option<CommentId>,
                search: Option<String>,
            }

            // Store issue and comment selection across app runs in order to
            // preselect them when re-running the app.
            let mut state = PreviousState::default();

            loop {
                let profile = ctx.profile()?;
                let me = profile.did();
                let rid = options.repo.unwrap_or(rid);
                let repository = profile.storage.repository(rid)?;

                let context = list::Context {
                    profile,
                    repository,
                    filter: (me, opts.filter.clone()).into(),
                    search: state.search.clone(),
                    issue: state.issue_id,
                    comment: state.comment_id,
                };

                let tui = list::Tui::new(context, terminal_info.clone());
                let selection = tui.run().await?;

                if opts.json {
                    let selection = selection
                        .map(|o| serde_json::to_string(&o).unwrap_or_default())
                        .unwrap_or_default();

                    log::info!("Exiting issue listing interface..");

                    eprint!("{selection}");
                } else if let Some(selection) = selection {
                    if let Some(operation) = selection.operation.clone() {
                        match operation {
                            IssueOperation::Show { id } => {
                                terminal::run_rad(
                                    Some("issue"),
                                    &["show".into(), id.to_string().into()],
                                )?;
                                break;
                            }
                            IssueOperation::Edit {
                                id,
                                comment_id,
                                search,
                            } => {
                                state = PreviousState {
                                    issue_id: Some(id),
                                    comment_id,
                                    search: Some(search),
                                };
                                match comment_id {
                                    Some(comment_id) => {
                                        terminal::run_rad(
                                            Some("issue"),
                                            &[
                                                "comment".into(),
                                                id.to_string().into(),
                                                "--edit".into(),
                                                comment_id.to_string().into(),
                                            ],
                                        )?;
                                    }
                                    _ => {
                                        terminal::run_rad(
                                            Some("issue"),
                                            &["edit".into(), id.to_string().into()],
                                        )?;
                                    }
                                }
                            }
                            IssueOperation::Solve { id, search } => {
                                state = PreviousState {
                                    issue_id: Some(id),
                                    comment_id: None,
                                    search: Some(search),
                                };
                                terminal::run_rad(
                                    Some("issue"),
                                    &["state".into(), id.to_string().into(), "--solved".into()],
                                )?;
                            }
                            IssueOperation::Close { id, search } => {
                                state = PreviousState {
                                    issue_id: Some(id),
                                    comment_id: None,
                                    search: Some(search),
                                };
                                terminal::run_rad(
                                    Some("issue"),
                                    &["state".into(), id.to_string().into(), "--closed".into()],
                                )?;
                            }
                            IssueOperation::Reopen { id, search } => {
                                state = PreviousState {
                                    issue_id: Some(id),
                                    comment_id: None,
                                    search: Some(search),
                                };
                                terminal::run_rad(
                                    Some("issue"),
                                    &["state".into(), id.to_string().into(), "--open".into()],
                                )?;
                            }
                            IssueOperation::Comment {
                                id,
                                reply_to,
                                search,
                            } => {
                                let comment_id = comment(
                                    &tui.context().profile,
                                    &tui.context().repository,
                                    id,
                                    Message::Edit,
                                    reply_to,
                                )?;
                                state = PreviousState {
                                    issue_id: Some(id),
                                    comment_id: Some(comment_id),
                                    search: Some(search),
                                };
                            }
                        }
                    }
                } else {
                    break;
                }
            }
        }
        Operation::Other { args } => {
            terminal::run_rad(Some("issue"), &args)?;
        }
        Operation::Unknown { .. } => {
            anyhow::bail!("unknown operation provided");
        }
    }

    Ok(())
}

fn comment(
    profile: &Profile,
    repo: &storage::git::Repository,
    issue_id: IssueId,
    message: Message,
    reply_to: Option<CommentId>,
) -> Result<CommentId, anyhow::Error> {
    let mut issues = profile.issues_mut(repo)?;
    let signer = cli::terminal::signer(profile)?;
    let mut issue = issues.get_mut(&issue_id)?;
    let (root_comment_id, _) = issue.root();
    let body = terminal::prompt_comment(message, issue.thread(), reply_to, None)?;
    let comment_id = issue.comment(body, reply_to.unwrap_or(*root_comment_id), vec![], &signer)?;

    Ok(comment_id)
}

#[cfg(test)]
mod test {
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
