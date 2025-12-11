use serde::Serialize;

use radicle::issue::IssueId;

/// The selected issue operation returned by the operation
/// selection widget.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum IssueOperation {
    Edit { id: IssueId },
    Show { id: IssueId },
}
