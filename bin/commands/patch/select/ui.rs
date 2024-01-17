use std::collections::HashMap;

use radicle::cob::patch::{Patch, PatchId};

use radicle_tui as tui;

use tui::context::Context;
use tui::ui::cob::PatchItem;
use tui::ui::theme::{style, Theme};
use tui::ui::widget::context::Shortcuts;
use tui::ui::widget::{Widget, WidgetComponent};

use tui::ui::widget::container::Tabs;
use tui::ui::widget::label::{self};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::tui::layout::Rect;
use tuirealm::{AttrValue, Attribute, Frame, MockComponent, Props, State};

use super::super::common;
use super::ListCid;

pub struct IdSelect {
    theme: Theme,
    browser: Widget<common::ui::PatchBrowser>,
}

impl IdSelect {
    pub fn new(theme: Theme, browser: Widget<common::ui::PatchBrowser>) -> Self {
        Self { theme, browser }
    }

    pub fn items(&self) -> &Vec<PatchItem> {
        self.browser.items()
    }

    pub fn shortcuts(&self) -> HashMap<ListCid, Widget<Shortcuts>> {
        [(
            ListCid::PatchBrowser,
            tui::ui::shortcuts(
                &self.theme,
                vec![
                    tui::ui::shortcut(&self.theme, "↑/↓", "navigate"),
                    tui::ui::shortcut(&self.theme, "enter", "show"),
                    tui::ui::shortcut(&self.theme, "q", "quit"),
                ],
            ),
        )]
        .iter()
        .cloned()
        .collect()
    }
}

impl WidgetComponent for IdSelect {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        self.browser.attr(Attribute::Focus, AttrValue::Flag(focus));
        self.browser.view(frame, area);
    }

    fn state(&self) -> State {
        self.browser.state()
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        self.browser.perform(cmd)
    }
}

pub struct OperationSelect {
    theme: Theme,
    browser: Widget<common::ui::PatchBrowser>,
}

impl OperationSelect {
    pub fn new(theme: Theme, browser: Widget<common::ui::PatchBrowser>) -> Self {
        Self { theme, browser }
    }

    pub fn items(&self) -> &Vec<PatchItem> {
        self.browser.items()
    }

    pub fn shortcuts(&self) -> HashMap<ListCid, Widget<Shortcuts>> {
        [(
            ListCid::PatchBrowser,
            tui::ui::shortcuts(
                &self.theme,
                vec![
                    tui::ui::shortcut(&self.theme, "↑/↓", "navigate"),
                    tui::ui::shortcut(&self.theme, "enter", "show"),
                    tui::ui::shortcut(&self.theme, "c", "checkout"),
                    tui::ui::shortcut(&self.theme, "e", "edit"),
                    tui::ui::shortcut(&self.theme, "q", "quit"),
                ],
            ),
        )]
        .iter()
        .cloned()
        .collect()
    }
}

impl WidgetComponent for OperationSelect {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        self.browser.attr(Attribute::Focus, AttrValue::Flag(focus));
        self.browser.view(frame, area);
    }

    fn state(&self) -> State {
        self.browser.state()
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        self.browser.perform(cmd)
    }
}

pub fn list_navigation(theme: &Theme) -> Widget<Tabs> {
    tui::ui::tabs(
        theme,
        vec![label::reversable("Patches").style(style::cyan())],
    )
}

pub fn id_select(
    context: &Context,
    theme: &Theme,
    selected: Option<(PatchId, Patch)>,
) -> Widget<IdSelect> {
    let browser = Widget::new(common::ui::PatchBrowser::new(context, theme, selected));

    Widget::new(IdSelect::new(theme.clone(), browser))
}

pub fn operation_select(
    context: &Context,
    theme: &Theme,
    selected: Option<(PatchId, Patch)>,
) -> Widget<OperationSelect> {
    let browser = Widget::new(common::ui::PatchBrowser::new(context, theme, selected));

    Widget::new(OperationSelect::new(theme.clone(), browser))
}
