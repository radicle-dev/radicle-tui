pub mod format;
pub mod im;
pub mod items;
pub mod rm;
pub mod span;

#[derive(Clone, Debug)]
pub struct TerminalInfo {
    pub luma: Option<f32>,
}

impl TerminalInfo {
    pub fn is_dark(&self) -> bool {
        self.luma.unwrap_or_default() <= 0.6
    }
}
