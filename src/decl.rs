use ratatui::layout::Direction;

#[derive(Default)]
pub struct App {}

pub struct PageView {}

impl PageView {
    pub fn new() -> Self {
        Self {}
    }
}

pub struct StackView {
    direction: Direction,
}

impl StackView {
    pub fn vertical() -> Self {
        Self {
            direction: Direction::Vertical,
        }
    }

    pub fn horizontal() -> Self {
        Self {
            direction: Direction::Horizontal,
        }
    }
}

pub struct ListView {}

impl ListView {
    pub fn new() -> Self {
        Self {}
    }
}
