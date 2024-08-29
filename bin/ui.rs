pub mod format;
pub mod items;
pub mod widget;
pub mod im;

#[derive(Clone, Debug)]
pub struct TerminalInfo {
    pub luma: Option<f32>,
}

impl TerminalInfo {
    pub fn is_dark(&self) -> bool {
        self.luma.unwrap_or_default() <= 0.6
    }
}
