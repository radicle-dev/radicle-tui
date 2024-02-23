use std::fmt::Display;

use serde::Serialize;

/// The application's mode. It tells the application
/// which widgets to render and which output to produce.
///
/// Depends on CLI arguments given by the user.
#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Operation,
    Id,
}

/// The selected patch operation returned by the operation
/// selection widget.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum PatchOperation {
    Show,
    Checkout,
    Delete,
    Edit,
    Comment,
}

impl Display for PatchOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatchOperation::Show => {
                write!(f, "show")
            }
            PatchOperation::Checkout => {
                write!(f, "checkout")
            }
            PatchOperation::Delete => {
                write!(f, "delete")
            }
            PatchOperation::Edit => {
                write!(f, "edit")
            }
            PatchOperation::Comment => {
                write!(f, "comment")
            }
        }
    }
}
