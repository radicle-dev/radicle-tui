use radicle::cob::patch::{Patch, PatchId};

use radicle_tui as tui;

use tui::context::Context;
use tui::ui::theme::{style, Theme};
use tui::ui::widget::Widget;

use tui::ui::widget::container::Tabs;
use tui::ui::widget::context::{ContextBar, Progress};
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

pub fn browse_context(context: &Context, _theme: &Theme, progress: Progress) -> Widget<ContextBar> {
    use radicle::cob::patch::State;

    let mut draft = 0;
    let mut open = 0;
    let mut archived = 0;
    let mut merged = 0;

    let patches = context.patches().as_ref().unwrap();
    for (_, patch) in patches {
        match patch.state() {
            State::Draft => draft += 1,
            State::Open { conflicts: _ } => open += 1,
            State::Archived => archived += 1,
            State::Merged {
                commit: _,
                revision: _,
            } => merged += 1,
        }
    }

    let context = label::default("");
    let divider = label::default(" | ");

    let draft_n = label::default(&format!("{draft}")).style(style::gray_dim());
    let draft = label::default(" Draft");

    let open_n = label::default(&format!("{open}")).style(style::green());
    let open = label::default(" Open");

    let archived_n = label::default(&format!("{archived}")).style(style::yellow());
    let archived = label::default(" Archived");

    let merged_n = label::default(&format!("{merged}")).style(style::cyan());
    let merged = label::default(" Merged ");

    let progress =
        label::default(&format!(" {} ", progress.to_string())).style(style::magenta_reversed());
    let spacer = label::default("");

    let context_bar = ContextBar::new(
        label::group(&[context]),
        label::group(&[spacer.clone()]),
        label::group(&[spacer]),
        label::group(&[
            draft_n,
            draft,
            divider.clone(),
            open_n,
            open,
            divider.clone(),
            archived_n,
            archived,
            divider,
            merged_n,
            merged,
        ]),
        label::group(&[progress]),
    );

    Widget::new(context_bar).height(1)
}
