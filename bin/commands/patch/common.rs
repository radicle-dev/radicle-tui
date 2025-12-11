use radicle::patch::PatchId;
use serde::Serialize;

/// The selected patch operation returned by the operation
/// selection widget.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum PatchOperation {
    Checkout { id: PatchId },
    Diff { id: PatchId },
    Show { id: PatchId },
    _Review { id: PatchId },
}
