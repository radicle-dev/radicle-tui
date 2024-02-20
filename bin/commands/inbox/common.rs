/// The application's subject. It tells the application
/// which widgets to render and which output to produce.
///
/// Depends on CLI arguments given by the user.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum Mode {
    Id,
    #[default]
    Operation,
}
