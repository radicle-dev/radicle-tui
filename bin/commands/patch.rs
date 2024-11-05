#[path = "patch/common.rs"]
mod common;
#[path = "patch/review.rs"]
mod review;
#[path = "patch/select.rs"]
mod select;

use std::ffi::OsString;

use anyhow::anyhow;

use radicle::cob;
use radicle::crypto::Signer;
use radicle::identity::RepoId;
use radicle::patch::{Status, Verdict};
use radicle::storage::git::cob::DraftStore;
use radicle::storage::WriteRepository;

use radicle_cli::git::Rev;
use radicle_cli::terminal;
use radicle_cli::terminal::args::{string, Args, Error, Help};

use crate::cob::patch;
use crate::cob::patch::Filter;
use crate::commands::tui_patch::review::ReviewAction;

use crate::tui_patch::review::builder::{Brain, CommentBuilder, ReviewBuilder};

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
    Review { opts: ReviewOptions },
}

#[derive(PartialEq, Eq)]
pub enum OperationName {
    Select,
    Review,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SelectOptions {
    mode: common::Mode,
    filter: patch::Filter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewOptions {
    patch_id: Rev,
    revision_id: Option<Rev>,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut op: Option<OperationName> = None;
        let mut repo = None;
        let mut select_opts = SelectOptions::default();
        let mut patch_id = None;
        let mut revision_id = None;

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
                    select_opts.filter = select_opts.filter.with_status(None);
                }
                Long("draft") if op == Some(OperationName::Select) => {
                    select_opts.filter = select_opts.filter.with_status(Some(Status::Draft));
                }
                Long("archived") if op == Some(OperationName::Select) => {
                    select_opts.filter = select_opts.filter.with_status(Some(Status::Archived));
                }
                Long("merged") if op == Some(OperationName::Select) => {
                    select_opts.filter = select_opts.filter.with_status(Some(Status::Merged));
                }
                Long("open") if op == Some(OperationName::Select) => {
                    select_opts.filter = select_opts.filter.with_status(Some(Status::Open));
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
                Long("revision") => {
                    let val = parser.value()?;
                    let rev_id = terminal::args::rev(&val)?;

                    revision_id = Some(rev_id);
                }
                Value(val) if op.is_none() => match val.to_string_lossy().as_ref() {
                    "select" => op = Some(OperationName::Select),
                    "review" => op = Some(OperationName::Review),
                    unknown => anyhow::bail!("unknown operation '{}'", unknown),
                },
                Value(val) if patch_id.is_none() => {
                    let val = string(&val);
                    patch_id = Some(Rev::from(val));
                }
                _ => return Err(anyhow!(arg.unexpected())),
            }
        }

        if select_opts.mode == common::Mode::Id {
            select_opts.filter = Filter::default().with_status(None)
        }

        let op = match op.ok_or_else(|| anyhow!("an operation must be provided"))? {
            OperationName::Review => Operation::Review {
                opts: ReviewOptions {
                    patch_id: patch_id.ok_or_else(|| anyhow!("a patch must be provided"))?,
                    revision_id: revision_id,
                },
            },
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

    match options.op {
        Operation::Select { opts } => {
            let profile = ctx.profile()?;
            let rid = options.repo.unwrap_or(rid);
            let repository = profile.storage.repository(rid).unwrap();

            if let Err(err) = crate::log::enable() {
                println!("{}", err);
            }
            log::info!("Starting patch selection interface in project {}..", rid);

            let context = select::Context {
                profile,
                repository,
                mode: opts.mode,
                filter: opts.filter.clone(),
            };
            let output = select::App::new(context, true).run().await?;

            let output = output
                .map(|o| serde_json::to_string(&o).unwrap_or_default())
                .unwrap_or_default();

            log::info!("About to print to `stderr`: {}", output);
            log::info!("Exiting patch selection interface..");

            eprint!("{output}");
        }
        Operation::Review { opts } => {
            if let Err(err) = crate::log::enable() {
                println!("{}", err);
            }
            log::info!("Starting patch review interface in project {rid}..");

            let profile = ctx.profile()?;
            let signer = terminal::signer(&profile)?;
            let rid = options.repo.unwrap_or(rid);
            let repo = profile.storage.repository(rid).unwrap();

            // Load patch
            let patch_id = opts.patch_id.resolve(&repo.backend)?;
            let patch = patch::find(&profile, &repo, &patch_id)?
                .ok_or_else(|| anyhow!("Patch `{patch_id}` not found"))?;

            // Load revision
            let revision_id = opts
                .revision_id
                .map(|rev| rev.resolve::<radicle::git::Oid>(&repo.backend))
                .transpose()?
                .map(radicle::cob::patch::RevisionId::from);
            let (_, revision) = match revision_id {
                Some(id) => (
                    id,
                    patch
                        .revision(&id)
                        .ok_or_else(|| anyhow!("Patch revision `{id}` not found"))?,
                ),
                None => patch.latest(),
            };

            let brain = if let Ok(b) = Brain::load(patch_id, signer.public_key(), repo.raw()) {
                log::info!(
                    "Loaded existing brain {} for patch {}",
                    b.head().id(),
                    &patch_id
                );
                b
            } else {
                let base = repo.raw().find_commit((*revision.base()).into())?;
                Brain::new(patch_id, signer.public_key(), base, repo.raw())?
            };

            let builder = ReviewBuilder::new(patch_id, &signer, &repo);
            let queue = builder.queue(&brain, &revision)?;

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

            let (review_id, review) = if let Some((id, review)) = patch
                .clone()
                .reviews_of(revision.id())
                .find(|(_, review)| review.author().public_key() == signer.public_key())
            {
                // Review already started, resume.
                log::info!("Resuming review {id}..");
                (id.clone(), review.clone())
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

                // Load newly created review.
                match patch
                    .reviews_of(revision.id())
                    .find(|(_, review)| review.author().public_key() == signer.public_key())
                {
                    Some((id, review)) => (id.clone(), review.clone()),
                    None => anyhow::bail!("Could not find review {}", id),
                }
            };

            loop {
                log::info!(
                    "Found comments for {review_id}: {:?}",
                    review.comments().collect::<Vec<_>>()
                );

                let selection = review::Tui::new(profile.clone(), rid, queue.clone())
                    .run()
                    .await?;
                log::info!("Received selection from TUI: {:?}", selection);

                if let Some(selection) = selection.as_ref() {
                    match ReviewAction::try_from(selection.action)? {
                        ReviewAction::Accept => {
                            // brain accept
                        }
                        ReviewAction::Ignore => {
                            // next hunk
                        }
                        ReviewAction::Comment => {
                            if let Some(hunk) = selection.hunk {
                                if let Some((_, item)) = queue.get(hunk) {
                                    let (old, new) = item.paths();
                                    let path = old.or(new);

                                    if let (Some(hunk), Some((path, _))) = (item.hunk(), path) {
                                        let builder = CommentBuilder::new(
                                            revision.head(),
                                            path.to_path_buf(),
                                        );
                                        let comments = builder.edit(hunk)?;

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
                                        log::warn!(
                                            "Commenting on binary blobs is not yet implemented"
                                        );
                                    }
                                } else {
                                    log::error!("Expected a hunk to comment on.")
                                }
                            } else {
                                log::error!("Expected a selected hunk.")
                            }
                        }
                    }
                } else {
                    break;
                }
            }
        }
    }

    Ok(())
}
