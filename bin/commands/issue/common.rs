use serde::Serialize;

use radicle::issue::IssueId;

/// The application's mode. It tells the application
/// which widgets to render and which output to produce.
/// Depends on CLI arguments given by the user.
#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Operation,
    Id,
}

/// The selected issue operation returned by the operation
/// selection widget.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum IssueOperation {
    Edit { id: IssueId },
    Show { id: IssueId },
}
