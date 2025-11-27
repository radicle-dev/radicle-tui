use std::ffi::OsString;
use std::process;

use thiserror::Error;

use radicle::cob::thread;
use radicle::git;

use radicle_cli::terminal;
use radicle_cli::terminal::args;
use radicle_cli::terminal::io;
use radicle_cli::terminal::patch::Message;
use radicle_cli::terminal::{Args, Command, DefaultContext, Error, Help};

#[derive(Error, Debug)]
pub enum ForwardError {
    #[error("an internal error occured while executing 'rad'")]
    RadInternal,
    #[error("an I/O error occured while trying to forward command to 'rad': {0}")]
    Io(#[from] std::io::Error),
}

fn _run_rad(args: &[OsString]) -> Result<(), ForwardError> {
    let status = process::Command::new("rad").args(args).status();

    match status {
        Ok(status) => {
            if !status.success() {
                return Err(ForwardError::RadInternal);
            }
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}

pub fn run_rad(command: Option<&str>, args: &[OsString]) -> Result<(), ForwardError> {
    let args = if let Some(command) = command {
        [vec![command.into()], args.to_vec()].concat()
    } else {
        args.to_vec()
    };

    _run_rad(&args)
}

pub fn run_command_args<A, C>(help: Help, cmd: C, args: Vec<OsString>) -> !
where
    A: Args,
    C: Command<A, DefaultContext>,
{
    use io as term;

    let options = match A::from_args(args) {
        Ok((opts, unparsed)) => {
            if let Err(err) = args::finish(unparsed) {
                term::error(err);
                process::exit(1);
            }
            opts
        }
        Err(err) => {
            let hint = match err.downcast_ref::<Error>() {
                Some(Error::Help) => {
                    help.print();
                    process::exit(0);
                }
                // Print the manual, or the regular help if there's an error.
                Some(Error::HelpManual { name }) => {
                    let Ok(status) = term::manual(name) else {
                        help.print();
                        process::exit(0);
                    };
                    if !status.success() {
                        help.print();
                        process::exit(0);
                    }
                    process::exit(status.code().unwrap_or(0));
                }
                Some(Error::Usage) => {
                    term::usage(help.name, help.usage);
                    process::exit(1);
                }
                Some(Error::WithHint { hint, .. }) => Some(hint),
                None => None,
            };
            io::error(format!("rad-tui {}: {err}", help.name));

            if let Some(hint) = hint {
                io::hint(hint);
            }
            process::exit(1);
        }
    };

    match cmd.run(options, DefaultContext) {
        Ok(()) => process::exit(0),
        Err(err) => {
            terminal::fail(help.name, &err);
            process::exit(1);
        }
    }
}

/// Get a comment from the user.
pub fn prompt_comment(
    message: Message,
    thread: &thread::Thread,
    mut reply_to: Option<git::Oid>,
    edit: Option<&str>,
) -> anyhow::Result<String> {
    let (chase, missing) = {
        let mut chase = Vec::with_capacity(thread.len());
        let mut missing = None;
        while let Some(id) = reply_to {
            if let Some(comment) = thread.comment(&id) {
                chase.push(comment);
                reply_to = comment.reply_to();
            } else {
                missing = reply_to;
                break;
            }
        }

        (chase, missing)
    };

    let quotes = if chase.is_empty() {
        ""
    } else {
        "Quotes (lines starting with '>') will be preserved. Please remove those that you do not intend to keep.\n"
    };

    let mut buffer = terminal::format::html::commented(format!("HTML comments, such as this one, are deleted before posting.\n{quotes}Saving an empty file aborts the operation.").as_str());
    buffer.push('\n');

    for comment in chase.iter().rev() {
        buffer.reserve(2);
        buffer.push('\n');
        comment_quoted(comment, &mut buffer);
    }

    if let Some(id) = missing {
        buffer.push('\n');
        buffer.push_str(
            terminal::format::html::commented(
                format!("The comment with ID {id} that was replied to could not be found.")
                    .as_str(),
            )
            .as_str(),
        );
    }

    if let Some(edit) = edit {
        if !chase.is_empty() {
            buffer.push_str(
                "\n<!-- The contents of the comment you are editing follow below this line. -->\n",
            );
        }

        buffer.reserve(2 + edit.len());
        buffer.push('\n');
        buffer.push_str(edit);
    }

    let body = message.get(&buffer)?;
    if body.is_empty() {
        anyhow::bail!("aborting operation due to empty comment");
    }

    Ok(body)
}

fn comment_quoted(comment: &thread::Comment, buffer: &mut String) {
    let body = comment.body();
    let lines = body.lines();
    let hint = {
        let (lower, upper) = lines.size_hint();
        upper.unwrap_or(lower)
    };

    buffer.push_str(format!("{} wrote:\n", comment.author()).as_str());
    buffer.reserve(body.len() + hint * 2);

    for line in lines {
        buffer.push('>');
        if !line.is_empty() {
            buffer.push(' ');
        }

        buffer.push_str(line);
        buffer.push('\n');
    }
}
