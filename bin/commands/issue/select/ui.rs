use std::collections::HashMap;

use radicle::issue::{Issue, IssueId};

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::tui::layout::Rect;
use tuirealm::{AttrValue, Attribute, Frame, MockComponent, Props, State};

use radicle_tui as tui;

use tui::cob::issue::Filter;
use tui::context::Context;
use tui::ui::cob::IssueItem;
use tui::ui::theme::{style, Theme};
use tui::ui::widget::container::Tabs;
use tui::ui::widget::context::Shortcuts;
use tui::ui::widget::label::{self};
use tui::ui::widget::{Widget, WidgetComponent};

use super::super::common;
use super::ListCid;

pub struct IdSelect {
    theme: Theme,
    browser: Widget<common::ui::IssueBrowser>,
}

impl IdSelect {
    pub fn new(theme: Theme, browser: Widget<common::ui::IssueBrowser>) -> Self {
        Self { theme, browser }
    }

    pub fn items(&self) -> &Vec<IssueItem> {
        self.browser.items()
    }

    pub fn shortcuts(&self) -> HashMap<ListCid, Widget<Shortcuts>> {
        [(
            ListCid::PatchBrowser,
            tui::ui::shortcuts(
                &self.theme,
                vec![
                    tui::ui::shortcut(&self.theme, "enter", "select"),
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
    browser: Widget<common::ui::IssueBrowser>,
}

impl OperationSelect {
    pub fn new(theme: Theme, browser: Widget<common::ui::IssueBrowser>) -> Self {
        Self { theme, browser }
    }

    pub fn items(&self) -> &Vec<IssueItem> {
        self.browser.items()
    }

    pub fn shortcuts(&self) -> HashMap<ListCid, Widget<Shortcuts>> {
        [(
            ListCid::PatchBrowser,
            tui::ui::shortcuts(
                &self.theme,
                vec![
                    tui::ui::shortcut(&self.theme, "enter", "show"),
                    tui::ui::shortcut(&self.theme, "m", "comment"),
                    tui::ui::shortcut(&self.theme, "e", "edit"),
                    tui::ui::shortcut(&self.theme, "d", "delete"),
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
    theme: &Theme,
    context: &Context,
    filter: Filter,
    selected: Option<(IssueId, Issue)>,
) -> Widget<IdSelect> {
    let browser = Widget::new(common::ui::IssueBrowser::new(
        theme, context, filter, selected,
    ));

    Widget::new(IdSelect::new(theme.clone(), browser))
}

pub fn operation_select(
    theme: &Theme,
    context: &Context,
    filter: Filter,
    selected: Option<(IssueId, Issue)>,
) -> Widget<OperationSelect> {
    let browser = Widget::new(common::ui::IssueBrowser::new(
        theme, context, filter, selected,
    ));

    Widget::new(OperationSelect::new(theme.clone(), browser))
}
