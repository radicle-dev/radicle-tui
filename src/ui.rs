pub mod ext;
pub mod im;
pub mod layout;
pub mod rm;
pub mod span;
pub mod theme;

use ratatui::layout::Constraint;
use ratatui::text::Text;

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
