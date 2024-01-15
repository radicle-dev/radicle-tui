use radicle::cob::patch::{Patch, PatchId};

use radicle_tui as tui;

use tui::context::Context;
use tui::ui::theme::{style, Theme};
use tui::ui::widget::Widget;

use tui::ui::widget::container::Tabs;
use tui::ui::widget::label::{self};

use super::super::common;

pub fn list_navigation(theme: &Theme) -> Widget<Tabs> {
    tui::ui::tabs(
        theme,
        vec![label::reversable("Patches").style(style::cyan())],
    )
}

pub fn patches(
    context: &Context,
    theme: &Theme,
    selected: Option<(PatchId, Patch)>,
) -> Widget<common::ui::PatchBrowser> {
    Widget::new(common::ui::PatchBrowser::new(context, theme, selected))
}
