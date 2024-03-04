use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::symbols;
use ratatui::widgets::{BorderType, Borders, Widget};

pub struct HeaderBlock {
    /// Visible borders
    borders: Borders,
    /// Border style
    border_style: Style,
    /// Type of the border. The default is plain lines but one can choose to have rounded corners
    /// or doubled lines instead.
    border_type: BorderType,
    /// Widget style
    style: Style,
}

impl Default for HeaderBlock {
    fn default() -> HeaderBlock {
        HeaderBlock {
            borders: Borders::NONE,
            border_style: Default::default(),
            border_type: BorderType::Rounded,
            style: Default::default(),
        }
    }
}

impl HeaderBlock {
    pub fn border_style(mut self, style: Style) -> HeaderBlock {
        self.border_style = style;
        self
    }

    pub fn style(mut self, style: Style) -> HeaderBlock {
        self.style = style;
        self
    }

    pub fn borders(mut self, flag: Borders) -> HeaderBlock {
        self.borders = flag;
        self
    }

    pub fn border_type(mut self, border_type: BorderType) -> HeaderBlock {
        self.border_type = border_type;
        self
    }
}

impl Widget for HeaderBlock {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.area() == 0 {
            return;
        }
        buf.set_style(area, self.style);
        let symbols = BorderType::to_border_set(self.border_type);

        // Sides
        if self.borders.intersects(Borders::LEFT) {
            for y in area.top()..area.bottom() {
                buf.get_mut(area.left(), y)
                    .set_symbol(symbols.vertical_left)
                    .set_style(self.border_style);
            }
        }
        if self.borders.intersects(Borders::TOP) {
            for x in area.left()..area.right() {
                buf.get_mut(x, area.top())
                    .set_symbol(symbols.horizontal_top)
                    .set_style(self.border_style);
            }
        }
        if self.borders.intersects(Borders::RIGHT) {
            let x = area.right() - 1;
            for y in area.top()..area.bottom() {
                buf.get_mut(x, y)
                    .set_symbol(symbols.vertical_right)
                    .set_style(self.border_style);
            }
        }
        if self.borders.intersects(Borders::BOTTOM) {
            let y = area.bottom() - 1;
            for x in area.left()..area.right() {
                buf.get_mut(x, y)
                    .set_symbol(symbols.horizontal_bottom)
                    .set_style(self.border_style);
            }
        }

        // Corners
        if self.borders.contains(Borders::RIGHT | Borders::BOTTOM) {
            buf.get_mut(area.right() - 1, area.bottom() - 1)
                .set_symbol(symbols::line::VERTICAL_LEFT)
                .set_style(self.border_style);
        }
        if self.borders.contains(Borders::RIGHT | Borders::TOP) {
            buf.get_mut(area.right() - 1, area.top())
                .set_symbol(symbols.top_right)
                .set_style(self.border_style);
        }
        if self.borders.contains(Borders::LEFT | Borders::BOTTOM) {
            buf.get_mut(area.left(), area.bottom() - 1)
                .set_symbol(symbols::line::VERTICAL_RIGHT)
                .set_style(self.border_style);
        }
        if self.borders.contains(Borders::LEFT | Borders::TOP) {
            buf.get_mut(area.left(), area.top())
                .set_symbol(symbols.top_left)
                .set_style(self.border_style);
        }
    }
}

#[derive(Clone)]
pub enum FooterBlockType {
    Single,
    Begin,
    End,
    Repeat,
}

pub struct FooterBlock {
    /// Visible borders
    borders: Borders,
    /// Border style
    border_style: Style,
    /// Type of the border. The default is plain lines but one can choose to have rounded corners
    /// or doubled lines instead.
    border_type: BorderType,
    ///
    block_type: FooterBlockType,
    /// Widget style
    style: Style,
}

impl Default for FooterBlock {
    fn default() -> Self {
        Self {
            block_type: FooterBlockType::Single,
            borders: Self::borders(FooterBlockType::Single),
            border_style: Default::default(),
            border_type: BorderType::Rounded,
            style: Default::default(),
        }
    }
}

impl FooterBlock {
    pub fn border_style(mut self, style: Style) -> Self {
        self.border_style = style;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn block_type(mut self, block_type: FooterBlockType) -> Self {
        self.block_type = block_type.clone();
        self.borders = Self::borders(block_type);
        self
    }

    pub fn border_type(mut self, border_type: BorderType) -> Self {
        self.border_type = border_type;
        self
    }

    fn borders(block_type: FooterBlockType) -> Borders {
        match block_type {
            FooterBlockType::Single | FooterBlockType::Begin => Borders::ALL,
            FooterBlockType::End | FooterBlockType::Repeat => {
                Borders::TOP | Borders::RIGHT | Borders::BOTTOM
            }
        }
    }
}

impl Widget for FooterBlock {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.area() == 0 {
            return;
        }
        buf.set_style(area, self.style);
        let symbols = BorderType::to_border_set(self.border_type);

        // Sides
        if self.borders.intersects(Borders::LEFT) {
            for y in area.top()..area.bottom() {
                buf.get_mut(area.left(), y)
                    .set_symbol(symbols.vertical_left)
                    .set_style(self.border_style);
            }
        }
        if self.borders.intersects(Borders::TOP) {
            for x in area.left()..area.right() {
                buf.get_mut(x, area.top())
                    .set_symbol(symbols.horizontal_top)
                    .set_style(self.border_style);
            }
        }
        if self.borders.intersects(Borders::RIGHT) {
            let x = area.right() - 1;
            for y in area.top()..area.bottom() {
                buf.get_mut(x, y)
                    .set_symbol(symbols.vertical_right)
                    .set_style(self.border_style);
            }
        }
        if self.borders.intersects(Borders::BOTTOM) {
            let y = area.bottom() - 1;
            for x in area.left()..area.right() {
                buf.get_mut(x, y)
                    .set_symbol(symbols.horizontal_bottom)
                    .set_style(self.border_style);
            }
        }

        // Corners
        if self.borders.contains(Borders::RIGHT | Borders::BOTTOM) {
            let symbol = match self.block_type {
                FooterBlockType::Begin | FooterBlockType::Repeat => symbols::line::HORIZONTAL_UP,
                _ => symbols.bottom_right,
            };
            buf.get_mut(area.right() - 1, area.bottom() - 1)
                .set_symbol(symbol)
                .set_style(self.border_style);
        }
        if self.borders.contains(Borders::RIGHT | Borders::TOP) {
            let symbol = match self.block_type {
                FooterBlockType::Begin | FooterBlockType::Repeat => symbols::line::HORIZONTAL_DOWN,
                _ => symbols::line::VERTICAL_LEFT,
            };
            buf.get_mut(area.right() - 1, area.top())
                .set_symbol(symbol)
                .set_style(self.border_style);
        }
        if self.borders.contains(Borders::LEFT | Borders::BOTTOM) {
            buf.get_mut(area.left(), area.bottom() - 1)
                .set_symbol(symbols.bottom_left)
                .set_style(self.border_style);
        }
        if self.borders.contains(Borders::LEFT | Borders::TOP) {
            buf.get_mut(area.left(), area.top())
                .set_symbol(symbols::line::VERTICAL_RIGHT)
                .set_style(self.border_style);
        }
    }
}
