use serde::Serialize;

use radicle::{cob::thread::CommentId, issue::IssueId};

/// The selected issue operation returned by the operation
/// selection widget.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum IssueOperation {
    Edit {
        id: IssueId,
        comment_id: Option<CommentId>,
        search: String,
    },
    Show {
        id: IssueId,
    },
    Close {
        id: IssueId,
    },
    Solve {
        id: IssueId,
    },
    Reopen {
        id: IssueId,
    },
    Comment {
        id: IssueId,
        reply_to: Option<CommentId>,
        search: String,
    },
}
