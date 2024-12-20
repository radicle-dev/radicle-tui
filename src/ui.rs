pub mod ext;
pub mod im;
pub mod layout;
pub mod rm;
pub mod span;
pub mod theme;
pub mod utils;

use ratatui::layout::Constraint;
use ratatui::text::Text;
use ratatui::widgets::Cell;

use tui_tree_widget::TreeItem;

pub const RENDER_WIDTH_XSMALL: usize = 50;
pub const RENDER_WIDTH_SMALL: usize = 70;
pub const RENDER_WIDTH_MEDIUM: usize = 150;
pub const RENDER_WIDTH_LARGE: usize = usize::MAX;

#[derive(Clone, Debug, Default)]
pub struct ColumnView {
    small: bool,
    medium: bool,
    large: bool,
}

impl ColumnView {
    pub fn all() -> Self {
        Self {
            small: true,
            medium: true,
            large: true,
        }
    }

    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    pub fn medium(mut self) -> Self {
        self.medium = true;
        self
    }

    pub fn large(mut self) -> Self {
        self.large = true;
        self
    }
}

#[derive(Clone, Debug)]
pub struct Column<'a> {
    pub text: Text<'a>,
    pub width: Constraint,
    pub skip: bool,
    pub view: ColumnView,
}

impl<'a> Column<'a> {
    pub fn new(text: impl Into<Text<'a>>, width: Constraint) -> Self {
        Self {
            text: text.into(),
            width,
            skip: false,
            view: ColumnView::all(),
        }
    }

    pub fn skip(mut self, skip: bool) -> Self {
        self.skip = skip;
        self
    }

    pub fn hide_small(mut self) -> Self {
        self.view = ColumnView::default().medium().large();
        self
    }

    pub fn hide_medium(mut self) -> Self {
        self.view = ColumnView::default().large();
        self
    }

    pub fn displayed(&self, area_width: usize) -> bool {
        if area_width < RENDER_WIDTH_SMALL {
            self.view.small
        } else if area_width < RENDER_WIDTH_MEDIUM {
            self.view.medium
        } else if area_width < RENDER_WIDTH_LARGE {
            self.view.large
        } else {
            true
        }
    }
}

/// Needs to be implemented for items that are supposed to be rendered in tables.
pub trait ToRow<const W: usize> {
    fn to_row(&self) -> [Cell; W];
}

/// Needs to be implemented for items that are supposed to be rendered in trees.
pub trait ToTree<Id>
where
    Id: ToString,
{
    fn rows(&self) -> Vec<TreeItem<'_, Id>>;
}

/// A `BufferedValue` that writes updates to an internal
/// buffer. This buffer can be applied or reset.
///
/// Reading from a `BufferedValue` will return the buffer if it's
/// not empty. It will return the actual value otherwise.
#[derive(Clone, Debug)]
pub struct BufferedValue<T>
where
    T: Clone,
{
    value: T,
    buffer: Option<T>,
}

impl<T> BufferedValue<T>
where
    T: Clone,
{
    pub fn new(value: T) -> Self {
        Self {
            value,
            buffer: None,
        }
    }

    pub fn apply(&mut self) {
        if let Some(buffer) = self.buffer.clone() {
            self.value = buffer;
        }
        self.buffer = None;
    }

    pub fn reset(&mut self) {
        self.buffer = None;
    }

    pub fn write(&mut self, value: T) {
        self.buffer = Some(value);
    }

    pub fn read(&self) -> T {
        if let Some(buffer) = self.buffer.clone() {
            buffer
        } else {
            self.value.clone()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn state_value_read_should_succeed() {
        let value = BufferedValue::new(0);
        assert_eq!(value.read(), 0);
    }

    #[test]
    fn state_value_read_buffer_should_succeed() {
        let mut value = BufferedValue::new(0);
        value.write(1);

        assert_eq!(value.read(), 1);
    }

    #[test]
    fn state_value_apply_should_succeed() {
        let mut value = BufferedValue::new(0);

        value.write(1);
        assert_eq!(value.read(), 1);

        value.apply();
        assert_eq!(value.read(), 1);
    }

    #[test]
    fn state_value_reset_should_succeed() {
        let mut value = BufferedValue::new(0);

        value.write(1);
        assert_eq!(value.read(), 1);

        value.reset();
        assert_eq!(value.read(), 0);
    }

    #[test]
    fn state_value_reset_after_apply_should_succeed() {
        let mut value = BufferedValue::new(0);

        value.write(1);
        assert_eq!(value.read(), 1);

        value.apply();
        value.reset();
        assert_eq!(value.read(), 1);
    }
}
