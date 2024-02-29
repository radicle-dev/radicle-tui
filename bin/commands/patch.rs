#[path = "patch/common.rs"]
mod common;
#[cfg(feature = "flux")]
#[path = "patch/flux.rs"]
mod flux;
#[cfg(feature = "realm")]
#[path = "patch/realm.rs"]
mod realm;

use std::ffi::OsString;

use anyhow::anyhow;

use radicle::identity::RepoId;
use radicle_tui as tui;

use tui::common::cob::patch::{self, State};
use tui::common::log;

use crate::terminal;
use crate::terminal::args::{Args, Error, Help};

pub const HELP: Help = Help {
    name: "patch",
    description: "Terminal interfaces for patches",
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad-tui patch select [<option>...]

Select options

    --mode <MODE>           Set selection mode; see MODE below (default: operation)
    --all                   Show all patches, including merged and archived patches
    --archived              Show only archived patches
    --merged                Show only merged patches
    --open                  Show only open patches (default)
    --draft                 Show only draft patches
    --authored              Show only patches that you have authored
    --author <did>          Show only patched where the given user is an author
                            (may be specified multiple times)

    The MODE argument can be 'operation' or 'id'. 'operation' selects a patch id and
    an operation, whereas 'id' selects a patch id only.
    

Other options

    --help              Print help
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
    filter: patch::Filter,
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
                Long("draft") if op == Some(OperationName::Select) => {
                    select_opts.filter = select_opts.filter.with_state(Some(State::Draft));
                }
                Long("archived") if op == Some(OperationName::Select) => {
                    select_opts.filter = select_opts.filter.with_state(Some(State::Archived));
                }
                Long("merged") if op == Some(OperationName::Select) => {
                    select_opts.filter = select_opts.filter.with_state(Some(State::Merged));
                }
                Long("open") if op == Some(OperationName::Select) => {
                    select_opts.filter = select_opts.filter.with_state(Some(State::Open));
                }
                Long("authored") if op == Some(OperationName::Select) => {
                    select_opts.filter = select_opts.filter.with_authored(true);
                }
                Long("author") if op == Some(OperationName::Select) => {
                    select_opts.filter = select_opts
                        .filter
                        .with_author(terminal::args::did(&parser.value()?)?);
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

#[cfg(feature = "realm")]
pub fn run(options: Options, _ctx: impl terminal::Context) -> anyhow::Result<()> {
    use tui::common::context;
    use tui::realm::Window;

    pub const FPS: u64 = 60;
    let (_, id) = radicle::rad::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;

    match options.op {
        Operation::Select { ref opts } => {
            let profile = terminal::profile()?;
            let context = context::Context::new(profile, id)?.with_patches();

            log::enable(context.profile(), "patch", "select")?;

            let mut app = realm::select::App::new(context, opts.mode.clone(), opts.filter.clone());
            let output = Window::default().run(&mut app, 1000 / FPS)?;

            let output = output
                .map(|o| serde_json::to_string(&o).unwrap_or_default())
                .unwrap_or_default();

            eprint!("{output}");
        }
    }

    Ok(())
}

#[cfg(feature = "flux")]
#[tokio::main]
pub async fn run(options: Options, _ctx: impl terminal::Context) -> anyhow::Result<()> {
    use radicle::storage::ReadStorage;

    let (_, rid) = radicle::rad::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;

    match options.op {
        Operation::Select { opts } => {
            let profile = terminal::profile()?;
            let rid = options.repo.unwrap_or(rid);
            let repository = profile.storage.repository(rid).unwrap();

            log::enable(&profile, "patch", "select")?;

            let context = flux::select::Context {
                profile,
                repository,
                mode: opts.mode,
                filter: opts.filter.clone(),
            };
            let output = flux::select::App::new(context).run().await?;

            let output = output
                .map(|o| serde_json::to_string(&o).unwrap_or_default())
                .unwrap_or_default();

            eprint!("{output}");
        }
    }

    Ok(())
}
