use radicle::cob::issue::{Issue, IssueId};
use radicle::cob::patch::{Patch, PatchId};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::tui::layout::Rect;
use tuirealm::{AttrValue, Attribute, Frame, MockComponent, Props, State};

use super::common;
use super::common::container::{LabeledContainer, Tabs};
use super::common::list::{ColumnWidth, Table};

use super::{Widget, WidgetComponent};

use crate::ui::cob::{IssueItem, PatchItem};
use crate::ui::context::Context;
use crate::ui::theme::Theme;

pub struct Dashboard {
    about: Widget<LabeledContainer>,
}

impl Dashboard {
    pub fn new(about: Widget<LabeledContainer>) -> Self {
        Self { about }
    }
}

impl WidgetComponent for Dashboard {
    fn view(&mut self, _properties: &Props, frame: &mut Frame, area: Rect) {
        self.about.view(frame, area);
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

pub struct IssueBrowser {
    items: Vec<IssueItem>,
    table: Widget<Table<IssueItem, 7>>,
}

impl IssueBrowser {
    pub fn new(context: &Context, theme: &Theme, selected: Option<(IssueId, Issue)>) -> Self {
        let header = [
            common::label(" ● "),
            common::label("ID"),
            common::label("Title"),
            common::label("Author"),
            common::label("Tags"),
            common::label("Assignees"),
            common::label("Opened"),
        ];

        let widths = [
            ColumnWidth::Fixed(3),
            ColumnWidth::Fixed(7),
            ColumnWidth::Grow,
            ColumnWidth::Fixed(21),
            ColumnWidth::Fixed(25),
            ColumnWidth::Fixed(21),
            ColumnWidth::Fixed(18),
        ];

        let repo = context.repository();
        let mut items = vec![];

        for (id, issue) in context.issues() {
            if let Ok(item) = IssueItem::try_from((context.profile(), repo, *id, issue.clone())) {
                items.push(item);
            }
        }

        items.sort_by(|a, b| b.timestamp().cmp(a.timestamp()));
        items.sort_by(|a, b| b.state().cmp(a.state()));

        let selected =
            selected.map(|(id, issue)| IssueItem::from((context.profile(), repo, id, issue)));

        let table = Widget::new(Table::new(&items, selected, header, widths, theme.clone()))
            .highlight(theme.colors.item_list_highlighted_bg);

        Self { items, table }
    }

    pub fn items(&self) -> &Vec<IssueItem> {
        &self.items
    }
}

impl WidgetComponent for IssueBrowser {
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

pub struct PatchBrowser {
    items: Vec<PatchItem>,
    table: Widget<Table<PatchItem, 8>>,
}

impl PatchBrowser {
    pub fn new(context: &Context, theme: &Theme, selected: Option<(PatchId, Patch)>) -> Self {
        let header = [
            common::label(" ● "),
            common::label("ID"),
            common::label("Title"),
            common::label("Author"),
            common::label("Head"),
            common::label("+"),
            common::label("-"),
            common::label("Updated"),
        ];

        let widths = [
            ColumnWidth::Fixed(3),
            ColumnWidth::Fixed(7),
            ColumnWidth::Grow,
            ColumnWidth::Fixed(21),
            ColumnWidth::Fixed(7),
            ColumnWidth::Fixed(4),
            ColumnWidth::Fixed(4),
            ColumnWidth::Fixed(18),
        ];

        let repo = context.repository();
        let mut items = vec![];

        for (id, patch) in context.patches() {
            if let Ok(item) = PatchItem::try_from((context.profile(), repo, *id, patch.clone())) {
                items.push(item);
            }
        }

        items.sort_by(|a, b| b.timestamp().cmp(a.timestamp()));
        items.sort_by(|a, b| a.state().cmp(b.state()));

        let selected = selected
            .map(|(id, patch)| PatchItem::try_from((context.profile(), repo, id, patch)).unwrap());

        let table = Widget::new(Table::new(&items, selected, header, widths, theme.clone()))
            .highlight(theme.colors.item_list_highlighted_bg);

        Self { items, table }
    }

    pub fn items(&self) -> &Vec<PatchItem> {
        &self.items
    }
}

impl WidgetComponent for PatchBrowser {
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

pub fn navigation(theme: &Theme) -> Widget<Tabs> {
    common::tabs(
        theme,
        vec![
            common::reversable_label("dashboard").foreground(theme.colors.tabs_highlighted_fg),
            common::reversable_label("issues").foreground(theme.colors.tabs_highlighted_fg),
            common::reversable_label("patches").foreground(theme.colors.tabs_highlighted_fg),
        ],
    )
}

pub fn dashboard(context: &Context, theme: &Theme) -> Widget<Dashboard> {
    let about = common::labeled_container(
        theme,
        "about",
        common::property_list(
            theme,
            vec![
                common::property(theme, "id", &context.id().to_string()),
                common::property(theme, "name", context.project().name()),
                common::property(theme, "description", context.project().description()),
            ],
        )
        .to_boxed(),
    );
    let dashboard = Dashboard::new(about);

    Widget::new(dashboard)
}

pub fn patches(
    context: &Context,
    theme: &Theme,
    selected: Option<(PatchId, Patch)>,
) -> Widget<PatchBrowser> {
    Widget::new(PatchBrowser::new(context, theme, selected))
}

pub fn issues(
    context: &Context,
    theme: &Theme,
    selected: Option<(IssueId, Issue)>,
) -> Widget<IssueBrowser> {
    Widget::new(IssueBrowser::new(context, theme, selected))
}
