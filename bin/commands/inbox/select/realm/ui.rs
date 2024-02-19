use std::collections::HashMap;

use radicle::node::notifications::Notification;

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::tui::layout::Rect;
use tuirealm::{AttrValue, Attribute, Frame, MockComponent, Props, State};

use radicle_tui as tui;

use tui::common::cob::inbox::{Filter, SortBy};
use tui::common::context::Context;
use tui::realm::ui::cob::NotificationItem;
use tui::realm::ui::theme::{style, Theme};
use tui::realm::ui::widget::context::{ContextBar, Progress, Shortcuts};
use tui::realm::ui::widget::label::{self};
use tui::realm::ui::widget::list::{ColumnWidth, Table};
use tui::realm::ui::widget::{Widget, WidgetComponent};

use super::ListCid;

pub struct NotificationBrowser {
    items: Vec<NotificationItem>,
    table: Widget<Table<NotificationItem, 7>>,
}

impl NotificationBrowser {
    pub fn new(
        theme: &Theme,
        context: &Context,
        sort_by: SortBy,
        selected: Option<Notification>,
    ) -> Self {
        let header = [
            label::header(""),
            label::header(" ● "),
            label::header("Type"),
            label::header("Summary"),
            label::header("ID"),
            label::header("Status"),
            label::header("Updated"),
        ];
        let widths = [
            ColumnWidth::Fixed(5),
            ColumnWidth::Fixed(3),
            ColumnWidth::Fixed(6),
            ColumnWidth::Grow,
            ColumnWidth::Fixed(15),
            ColumnWidth::Fixed(10),
            ColumnWidth::Fixed(18),
        ];

        let mut items = vec![];
        for notification in context.notifications() {
            if let Ok(item) =
                NotificationItem::try_from((context.repository(), notification.clone()))
            {
                items.push(item);
            }
        }

        match sort_by.field {
            "timestamp" => items.sort_by(|a, b| b.timestamp().cmp(a.timestamp())),
            "id" => items.sort_by(|a, b| b.id().cmp(a.id())),
            _ => {}
        }
        if sort_by.reverse {
            items.reverse();
        }

        let selected = match selected {
            Some(notif) => {
                Some(NotificationItem::try_from((context.repository(), notif.clone())).unwrap())
            }
            _ => items.first().cloned(),
        };

        let table = Widget::new(Table::new(&items, selected, header, widths, theme.clone()));

        Self { items, table }
    }

    pub fn items(&self) -> &Vec<NotificationItem> {
        &self.items
    }
}

impl WidgetComponent for NotificationBrowser {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        self.table.attr(Attribute::Focus, AttrValue::Flag(focus));
        self.table.view(frame, area);
    }

    fn state(&self) -> State {
        self.table.state()
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        self.table.perform(cmd)
    }
}

pub struct IdSelect {
    theme: Theme,
    browser: Widget<NotificationBrowser>,
}

impl IdSelect {
    pub fn new(theme: Theme, browser: Widget<NotificationBrowser>) -> Self {
        Self { theme, browser }
    }

    pub fn items(&self) -> &Vec<NotificationItem> {
        self.browser.items()
    }

    pub fn shortcuts(&self) -> HashMap<ListCid, Widget<Shortcuts>> {
        [(
            ListCid::NotificationBrowser,
            tui::realm::ui::shortcuts(
                &self.theme,
                vec![
                    tui::realm::ui::shortcut(&self.theme, "enter", "select"),
                    tui::realm::ui::shortcut(&self.theme, "q", "quit"),
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
    browser: Widget<NotificationBrowser>,
}

impl OperationSelect {
    pub fn new(theme: Theme, browser: Widget<NotificationBrowser>) -> Self {
        Self { theme, browser }
    }

    pub fn items(&self) -> &Vec<NotificationItem> {
        self.browser.items()
    }

    pub fn shortcuts(&self) -> HashMap<ListCid, Widget<Shortcuts>> {
        [(
            ListCid::NotificationBrowser,
            tui::realm::ui::shortcuts(
                &self.theme,
                vec![
                    tui::realm::ui::shortcut(&self.theme, "enter", "show"),
                    tui::realm::ui::shortcut(&self.theme, "c", "clear"),
                    tui::realm::ui::shortcut(&self.theme, "q", "quit"),
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

pub fn id_select(
    theme: &Theme,
    context: &Context,
    _filter: Filter,
    sort_by: SortBy,
    selected: Option<Notification>,
) -> Widget<IdSelect> {
    let browser = Widget::new(NotificationBrowser::new(theme, context, sort_by, selected));

    Widget::new(IdSelect::new(theme.clone(), browser))
}

pub fn operation_select(
    theme: &Theme,
    context: &Context,
    _filter: Filter,
    sort_by: SortBy,
    selected: Option<Notification>,
) -> Widget<OperationSelect> {
    let browser = Widget::new(NotificationBrowser::new(theme, context, sort_by, selected));

    Widget::new(OperationSelect::new(theme.clone(), browser))
}

pub fn browse_context(
    _context: &Context,
    _theme: &Theme,
    _filter: Filter,
    progress: Progress,
) -> Widget<ContextBar> {
    let context = label::reversable("/").style(style::magenta_reversed());
    let filter = label::default("").style(style::magenta_dim());

    let progress = label::reversable(&progress.to_string()).style(style::magenta_reversed());

    let spacer = label::default("");
    let _divider = label::default(" | ");

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
