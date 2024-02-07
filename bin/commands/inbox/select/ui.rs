use std::collections::HashMap;

use radicle::issue::{Issue, IssueId};

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::tui::layout::Rect;
use tuirealm::{AttrValue, Attribute, Frame, MockComponent, Props, State};

use radicle_tui as tui;

use tui::cob::inbox::Filter;
use tui::context::Context;
use tui::ui::theme::{style, Theme};
use tui::ui::widget::context::{ContextBar, Progress, Shortcuts};
use tui::ui::widget::label::{self};
use tui::ui::widget::{Widget, WidgetComponent};

use super::ListCid;

pub struct NotificationBrowser {}

impl WidgetComponent for NotificationBrowser {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

pub struct OperationSelect {
    theme: Theme,
    browser: Widget<NotificationBrowser>,
}

impl OperationSelect {
    pub fn new(theme: Theme, browser: Widget<NotificationBrowser>) -> Self {
        Self { theme, browser }
    }

    pub fn shortcuts(&self) -> HashMap<ListCid, Widget<Shortcuts>> {
        [(
            ListCid::NotificationBrowser,
            tui::ui::shortcuts(
                &self.theme,
                vec![tui::ui::shortcut(&self.theme, "q", "quit")],
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

pub fn operation_select(
    theme: &Theme,
    context: &Context,
    filter: Filter,
    selected: Option<(IssueId, Issue)>,
) -> Widget<OperationSelect> {
    let browser = Widget::new(NotificationBrowser {});

    Widget::new(OperationSelect::new(theme.clone(), browser))
}

pub fn browse_context(
    context: &Context,
    _theme: &Theme,
    filter: Filter,
    progress: Progress,
) -> Widget<ContextBar> {
    let context = label::reversable("/").style(style::magenta_reversed());
    let filter = label::default("").style(style::magenta_dim());

    let progress = label::reversable(&progress.to_string()).style(style::magenta_reversed());

    let spacer = label::default("");
    let divider = label::default(" | ");

    let context_bar = ContextBar::new(
        label::group(&[context]),
        label::group(&[filter]),
        label::group(&[spacer.clone()]),
        label::group(&[
            spacer.clone(),
            spacer.clone(),
            spacer.clone(),
            spacer.clone(),
            spacer.clone(),
            spacer.clone(),
            spacer.clone(),
            spacer.clone(),
            spacer.clone(),
            spacer.clone(),
            spacer.clone(),
        ]),
        label::group(&[progress]),
    );

    Widget::new(context_bar).height(1)
}
