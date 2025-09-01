#[path = "patch/common.rs"]
mod common;
#[path = "patch/list.rs"]
mod list;
#[path = "patch/review.rs"]
mod review;

use std::ffi::OsString;

use anyhow::anyhow;

use radicle::cob::ObjectId;
use radicle::identity::RepoId;
use radicle::patch::{Patch, Revision, RevisionId, Status};

use radicle::storage::git::Repository;
use radicle_cli::git::Rev;
use radicle_cli::terminal;
use radicle_cli::terminal::args::{string, Args, Error, Help};

use crate::cob::patch;
use crate::cob::patch::Filter;

pub const HELP: Help = Help {
    name: "patch",
    description: "Terminal interfaces for patches",
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad-tui patch list [<option>...]

List options

    --mode <MODE>           Set selection mode; see MODE below (default: operation)
    --json                  Return JSON on stderr instead of calling `rad`

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
    Review { opts: ReviewOptions },
    Unknown { args: Vec<OsString> },
    Other { args: Vec<OsString> },
}

#[allow(dead_code)]
#[derive(PartialEq, Eq)]
pub enum OperationName {
    List,
    Review,
    Unknown,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ListOptions {
    mode: common::Mode,
    filter: patch::Filter,
    json: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReviewOptions {
    edit: bool,
    patch_id: Option<Rev>,
    revision_id: Option<Rev>,
}

impl ReviewOptions {
    pub fn revision_or_latest<'a>(
        &'a self,
        patch: &'a Patch,
        repo: &Repository,
    ) -> anyhow::Result<(RevisionId, &'a Revision)> {
        let revision_id = self
            .revision_id
            .as_ref()
            .map(|rev| rev.resolve::<radicle::git::Oid>(&repo.backend))
            .transpose()?
            .map(radicle::cob::patch::RevisionId::from);

        match revision_id {
            Some(id) => Ok((
                id,
                patch
                    .revision(&id)
                    .ok_or_else(|| anyhow!("Patch revision `{id}` not found"))?,
            )),
            None => Ok((patch.latest().0, patch.latest().1)),
        }
    }
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args.clone());
        let mut op = OperationName::List;
        let mut forward = None;
        let mut json = false;
        let mut help = false;
        let mut edit = false;
        let mut repo = None;
        let mut list_opts = ListOptions::default();
        let mut patch_id = None;
        let mut revision_id = None;

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
                    list_opts.filter = list_opts.filter.with_status(None);
                }
                Long("draft") if op == OperationName::List => {
                    list_opts.filter = list_opts.filter.with_status(Some(Status::Draft));
                }
                Long("archived") if op == OperationName::List => {
                    list_opts.filter = list_opts.filter.with_status(Some(Status::Archived));
                }
                Long("merged") if op == OperationName::List => {
                    list_opts.filter = list_opts.filter.with_status(Some(Status::Merged));
                }
                Long("open") if op == OperationName::List => {
                    list_opts.filter = list_opts.filter.with_status(Some(Status::Open));
                }
                Long("authored") if op == OperationName::List => {
                    list_opts.filter = list_opts.filter.with_authored(true);
                }
                Long("author") if op == OperationName::List => {
                    list_opts.filter = list_opts
                        .filter
                        .with_author(terminal::args::did(&parser.value()?)?);
                }
                Long("repo") => {
                    let val = parser.value()?;
                    let rid = terminal::args::rid(&val)?;

                    repo = Some(rid);
                }
                Long("revision") => {
                    let val = parser.value()?;
                    let rev_id = terminal::args::rev(&val)?;

                    revision_id = Some(rev_id);
                }
                Long("edit") => {
                    edit = true;
                }
                Value(val) if op == OperationName::List => match val.to_string_lossy().as_ref() {
                    "list" => op = OperationName::List,
                    "review" => op = OperationName::Review,
                    _ => {
                        op = OperationName::Unknown;
                        // Only enable forwarding if it was not already disabled explicitly
                        forward = match forward {
                            Some(false) => Some(false),
                            _ => Some(true),
                        };
                    }
                },
                Value(val) if patch_id.is_none() => {
                    let val = string(&val);
                    patch_id = Some(Rev::from(val));
                }
                _ => match op {
                    OperationName::List | OperationName::Review => {
                        return Err(anyhow!(arg.unexpected()));
                    }
                    _ => {}
                },
            }
        }

        // Disable forwarding if it was not enabled via `--help` or was
        // not disabled explicitly.
        let forward = forward.unwrap_or_default();

        // Show local help
        if help && !forward {
            return Err(Error::Help.into());
        }

        // Configure list options
        if list_opts.mode == common::Mode::Id {
            list_opts.filter = Filter::default().with_status(None)
        }
        list_opts.json = json;

        // Map local commands. Forward help and ignore `no-forward`.
        let op = match op {
            OperationName::Review if !forward => Operation::Review {
                opts: ReviewOptions {
                    edit,
                    patch_id,
                    revision_id,
                },
            },
            OperationName::List if !forward => Operation::List { opts: list_opts },
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

    if let Err(err) = crate::log::enable() {
        println!("{err}");
    }

    match options.op {
        Operation::List { opts } => {
            let profile = ctx.profile()?;
            let rid = options.repo.unwrap_or(rid);

            // Run TUI with patch list interface
            let selection = interface::list(opts.clone(), profile.clone(), rid).await?;

            if opts.json {
                let selection = selection
                    .map(|o| serde_json::to_string(&o).unwrap_or_default())
                    .unwrap_or_default();

                log::info!("About to print to `stderr`: {}", selection);
                log::info!("Exiting patch list interface..");

                eprint!("{selection}");
            } else if let Some(selection) = selection {
                if let Some(operation) = selection.operation.clone() {
                    let mut args = vec![operation.to_string()];

                    if let Some(id) = selection.ids.first() {
                        args.push(format!("{id}"));

                        match operation.as_str() {
                            "review" => {
                                let opts = ReviewOptions::default();
                                interface::review(opts, profile, rid, *id).await?;
                            }
                            _ => {
                                let args = args.into_iter().map(OsString::from).collect::<Vec<_>>();
                                let _ = crate::terminal::run_rad(Some("patch"), &args);
                            }
                        }
                    }
                }
            }
        }
        Operation::Review { ref opts } => {
            log::info!("Starting patch review interface in project {rid}..");

            let profile = ctx.profile()?;
            let rid = options.repo.unwrap_or(rid);
            let repo = profile.storage.repository(rid).unwrap();

            let patch_id: ObjectId = if let Some(patch_id) = &opts.patch_id {
                patch_id.resolve(&repo.backend)?
            } else {
                anyhow::bail!("a patch must be provided");
            };

            // Run TUI with patch review interface
            interface::review(opts.clone(), profile, rid, patch_id).await?;
        }
        Operation::Other { args } => {
            let _ = crate::terminal::run_rad(Some("patch"), &args);
        }
        Operation::Unknown { .. } => {
            anyhow::bail!("unknown operation provided");
        }
    }

    Ok(())
}

mod interface {
    use anyhow::anyhow;

    use radicle::cob;
    use radicle::cob::ObjectId;
    use radicle::identity::RepoId;
    use radicle::patch::PatchId;
    use radicle::patch::Verdict;
    use radicle::storage::git::cob::DraftStore;
    use radicle::storage::ReadStorage;
    use radicle::Profile;

    use radicle_cli::terminal;

    use radicle_tui::Selection;

    use crate::cob::patch;
    use crate::tui_patch::list;
    use crate::tui_patch::review::builder::CommentBuilder;
    use crate::tui_patch::review::ReviewAction;
    use crate::tui_patch::review::ReviewMode;

    use super::review;
    use super::review::builder::ReviewBuilder;
    use super::{ListOptions, ReviewOptions};

    pub async fn list(
        opts: ListOptions,
        profile: Profile,
        rid: RepoId,
    ) -> anyhow::Result<Option<Selection<ObjectId>>> {
        let repository = profile.storage.repository(rid).unwrap();

        log::info!("Starting patch selection interface in project {}..", rid);

        let context = list::Context {
            profile,
            repository,
            mode: opts.mode,
            filter: opts.filter.clone(),
        };

        list::App::new(context, true).run().await
    }

    pub async fn review(
        opts: ReviewOptions,
        profile: Profile,
        rid: RepoId,
        patch_id: PatchId,
    ) -> anyhow::Result<()> {
        let repo = profile.storage.repository(rid).unwrap();
        let signer = terminal::signer(&profile)?;

        let patch = patch::find(&profile, &repo, &patch_id)?
            .ok_or_else(|| anyhow!("Patch `{patch_id}` not found"))?;
        let (_, revision) = opts.revision_or_latest(&patch, &repo)?;
        let hunks = ReviewBuilder::new(&repo).hunks(revision)?;

        let drafts = DraftStore::new(&repo, *signer.public_key());
        let mut patches = cob::patch::Cache::no_cache(&drafts)?;
        let mut patch = patches.get_mut(&patch_id)?;

        if let Some(review) = revision.review_by(signer.public_key()) {
            // Review already finalized. Do nothing and warn.
            terminal::warning(format!(
                "Review ({}) already finalized. Exiting.",
                review.id()
            ));

            return Ok(());
        };

        let mode = if opts.edit {
            if let Some((id, _)) = patch::find_review(&patch, revision, &signer) {
                // Review already started, resume.
                log::info!("Resuming review {id}..");

                ReviewMode::Edit { resume: true }
            } else {
                // No review to resume, start a new one.
                let id = patch.review(
                    revision.id(),
                    // This is amended before the review is finalized, if all hunks are
                    // accepted. We can't set this to `None`, as that will be invalid without
                    // a review summary.
                    Some(Verdict::Reject),
                    None,
                    vec![],
                    &signer,
                )?;
                log::info!("Starting new review {id}..");

                ReviewMode::Edit { resume: false }
            }
        } else {
            ReviewMode::Show
        };

        loop {
            // Reload review
            let signer = profile.signer()?;
            let (review_id, review) = patch::find_review(&patch, revision, &signer)
                .ok_or_else(|| anyhow!("Could not find review."))?;

            let response = review::Tui::new(
                mode.clone(),
                profile.storage.clone(),
                rid,
                patch_id,
                patch.title().to_string(),
                revision.clone(),
                review.clone(),
                hunks.clone(),
            )
            .run()
            .await?;

            log::debug!("Received response from TUI: {:?}", response);

            if let Some(response) = response.as_ref() {
                if let Some(ReviewAction::Comment) = response.action {
                    let hunk = response
                        .state
                        .selected_hunk()
                        .ok_or_else(|| anyhow!("expected a selected hunk"))?;
                    let item = hunks
                        .get(hunk)
                        .ok_or_else(|| anyhow!("expected a hunk to comment on"))?;

                    let (old, new) = item.paths();
                    let path = old.or(new);

                    if let (Some(hunk), Some((path, _))) = (item.hunk(), path) {
                        let builder = CommentBuilder::new(revision.head(), path.to_path_buf());
                        let comments = builder.edit(hunk)?;

                        let signer = profile.signer()?;
                        patch.transaction("Review comments", &signer, |tx| {
                            for comment in comments {
                                tx.review_comment(
                                    review_id,
                                    comment.body,
                                    Some(comment.location),
                                    None,   // Not a reply.
                                    vec![], // No embeds.
                                )?;
                            }
                            Ok(())
                        })?;
                    } else {
                        log::warn!("Commenting on binary blobs is not yet implemented");
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(())
    }
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
