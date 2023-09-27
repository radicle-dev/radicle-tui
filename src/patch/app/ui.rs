use radicle::cob::patch::{Patch, PatchId};

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::tui::layout::Rect;
use tuirealm::{AttrValue, Attribute, Frame, MockComponent, Props, State};

use radicle_tui as tui;

use tui::ui::cob;
use tui::ui::cob::PatchItem;
use tui::ui::context::Context;
use tui::ui::layout;
use tui::ui::theme::Theme;
use tui::ui::widget::{Widget, WidgetComponent};

use tui::ui::widget::container::Tabs;
use tui::ui::widget::context::{ContextBar, Progress};
use tui::ui::widget::label::Label;
use tui::ui::widget::list::{ColumnWidth, Table};

pub struct PatchBrowser {
    items: Vec<PatchItem>,
    table: Widget<Table<PatchItem, 8>>,
}

impl PatchBrowser {
    pub fn new(context: &Context, theme: &Theme, selected: Option<(PatchId, Patch)>) -> Self {
        let header = [
            tui::ui::label(" ● "),
            tui::ui::label("ID"),
            tui::ui::label("Title"),
            tui::ui::label("Author"),
            tui::ui::label("Head"),
            tui::ui::label("+"),
            tui::ui::label("-"),
            tui::ui::label("Updated"),
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

        let selected = match selected {
            Some((id, patch)) => {
                Some(PatchItem::try_from((context.profile(), repo, id, patch)).unwrap())
            }
            _ => items.first().cloned(),
        };

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

pub struct Activity {
    label: Widget<Label>,
}

impl Activity {
    pub fn new(label: Widget<Label>) -> Self {
        Self { label }
    }
}

impl WidgetComponent for Activity {
    fn view(&mut self, _properties: &Props, frame: &mut Frame, area: Rect) {
        let label_w = self
            .label
            .query(Attribute::Width)
            .unwrap_or(AttrValue::Size(1))
            .unwrap_size();

        self.label
            .view(frame, layout::centered_label(label_w, area));
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

pub struct Files {
    label: Widget<Label>,
}

impl Files {
    pub fn new(label: Widget<Label>) -> Self {
        Self { label }
    }
}

impl WidgetComponent for Files {
    fn view(&mut self, _properties: &Props, frame: &mut Frame, area: Rect) {
        let label_w = self
            .label
            .query(Attribute::Width)
            .unwrap_or(AttrValue::Size(1))
            .unwrap_size();

        self.label
            .view(frame, layout::centered_label(label_w, area));
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

pub fn list_navigation(theme: &Theme) -> Widget<Tabs> {
    tui::ui::tabs(
        theme,
        vec![tui::ui::reversable_label("Patches").foreground(theme.colors.tabs_highlighted_fg)],
    )
}

pub fn navigation(theme: &Theme) -> Widget<Tabs> {
    tui::ui::tabs(
        theme,
        vec![
            tui::ui::reversable_label("Activity").foreground(theme.colors.tabs_highlighted_fg),
            tui::ui::reversable_label("Files").foreground(theme.colors.tabs_highlighted_fg),
        ],
    )
}

pub fn patches(
    context: &Context,
    theme: &Theme,
    selected: Option<(PatchId, Patch)>,
) -> Widget<PatchBrowser> {
    Widget::new(PatchBrowser::new(context, theme, selected))
}

pub fn activity(theme: &Theme) -> Widget<Activity> {
    let not_implemented = tui::ui::label("not implemented").foreground(theme.colors.default_fg);
    let activity = Activity::new(not_implemented);

    Widget::new(activity)
}

pub fn files(theme: &Theme) -> Widget<Files> {
    let not_implemented = tui::ui::label("not implemented").foreground(theme.colors.default_fg);
    let files = Files::new(not_implemented);

    Widget::new(files)
}

pub fn context(context: &Context, theme: &Theme, patch: (PatchId, Patch)) -> Widget<ContextBar> {
    let (id, patch) = patch;
    let (_, rev) = patch.latest();
    let is_you = *patch.author().id() == context.profile().did();

    let id = cob::format::cob(&id);
    let title = patch.title();
    let author = cob::format_author(patch.author().id(), is_you);
    let comments = rev.discussion().len();

    tui::ui::widget::context::bar(theme, "Patch", &id, title, &author, &comments.to_string())
}

pub fn browse_context(context: &Context, theme: &Theme, progress: Progress) -> Widget<ContextBar> {
    use radicle::cob::patch::State;

    let patches = context.patches();
    let mut draft = 0;
    let mut open = 0;
    let mut archived = 0;
    let mut merged = 0;

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

    tui::ui::widget::context::bar(
        theme,
        "Browse",
        "",
        "",
        &format!("{draft} draft | {open} open | {archived} archived | {merged} merged"),
        &progress.to_string(),
    )
}
