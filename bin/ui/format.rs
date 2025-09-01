use std::time::{SystemTime, UNIX_EPOCH};

use radicle::cob::Label;
use radicle::cob::{ObjectId, Timestamp};
use radicle::crypto::PublicKey;
use radicle::issue;
use radicle::node::Alias;
use radicle::patch;
use radicle::prelude::Did;
use ratatui::style::Color;

/// Format a git Oid.
pub fn oid(oid: impl Into<radicle::git::Oid>) -> String {
    format!("{:.7}", oid.into())
}

/// Format a COB id.
pub fn cob(id: &ObjectId) -> String {
    format!("{:.7}", id.to_string())
}

/// Format a DID.
pub fn did(did: &Did) -> String {
    let nid = did.as_key().to_human();
    format!("{}…{}", &nid[..7], &nid[nid.len() - 7..])
}

/// Format a timestamp.
pub fn timestamp(time: &Timestamp) -> String {
    let fmt = timeago::Formatter::new();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let now = Timestamp::from_secs(now.as_secs());
    let duration = std::time::Duration::from_secs(now.as_secs() - time.as_secs());

    fmt.convert(duration)
}

pub fn issue_state(state: &issue::State) -> (String, Color) {
    match state {
        issue::State::Open => (" ● ".into(), Color::Green),
        issue::State::Closed { reason: _ } => (" ● ".into(), Color::Red),
    }
}

pub fn patch_state(state: &patch::State) -> (String, Color) {
    match state {
        patch::State::Open { conflicts: _ } => (" ● ".into(), Color::Green),
        patch::State::Archived => (" ● ".into(), Color::Yellow),
        patch::State::Draft => (" ● ".into(), Color::Gray),
        patch::State::Merged {
            revision: _,
            commit: _,
        } => (" ✔ ".into(), Color::Magenta),
    }
}

pub fn labels(labels: &[Label]) -> String {
    let mut output = String::new();
    let mut labels = labels.iter().peekable();

    while let Some(label) = labels.next() {
        output.push_str(&label.to_string());

        if labels.peek().is_some() {
            output.push_str(", ");
        }
    }
    output
}

pub fn author(did: &Did, alias: &Option<Alias>, is_you: bool) -> String {
    let author = match alias {
        Some(alias) => format!("{alias}"),
        None => self::did(did),
    };

    if is_you {
        format!("{author} (you)")
    } else {
        author
    }
}

pub fn assignees(assignees: &[(Option<PublicKey>, Option<Alias>, bool)]) -> String {
    let mut output = String::new();
    let mut assignees = assignees.iter().peekable();

    while let Some((assignee, alias, is_you)) = assignees.next() {
        if let Some(assignee) = assignee {
            output.push_str(&self::author(&Did::from(assignee), alias, *is_you));
        }

        if assignees.peek().is_some() {
            output.push(',');
        }
    }
    output
}
