use std::fmt::Display;

use serde::Serialize;

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
    Edit,
    Show,
}

impl Display for IssueOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IssueOperation::Edit => {
                write!(f, "edit")
            }
            IssueOperation::Show => {
                write!(f, "show")
            }
        }
    }
}
