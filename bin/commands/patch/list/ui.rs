use radicle::cob::patch::{Patch, PatchId};

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::tui::layout::Rect;
use tuirealm::{AttrValue, Attribute, Frame, MockComponent, Props, State};

use radicle_tui as tui;

use tui::context::Context;
use tui::ui::cob::PatchItem;
use tui::ui::theme::Theme;
use tui::ui::widget::{Widget, WidgetComponent};

use tui::ui::widget::container::Tabs;
use tui::ui::widget::context::{ContextBar, Progress};
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
        let patches = context.patches().as_ref().unwrap();
        let mut items = vec![];

        for (id, patch) in patches {
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

pub fn list_navigation(theme: &Theme) -> Widget<Tabs> {
    tui::ui::tabs(
        theme,
        vec![tui::ui::reversable_label("Patches").foreground(theme.colors.tabs_highlighted_fg)],
    )
}

pub fn patches(
    context: &Context,
    theme: &Theme,
    selected: Option<(PatchId, Patch)>,
) -> Widget<PatchBrowser> {
    Widget::new(PatchBrowser::new(context, theme, selected))
}

pub fn browse_context(context: &Context, theme: &Theme, progress: Progress) -> Widget<ContextBar> {
    use radicle::cob::patch::State;
    use tui::ui::{label, label_group};

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

    let context = label(" Patches ")
        .foreground(theme.colors.context_bg)
        .background(theme.colors.context_badge_bg);

    let divider = label(" | ")
        .foreground(theme.colors.context_light)
        .background(theme.colors.context_bg);

    let draft_n = label(&format!("{draft}"))
        .foreground(tuirealm::props::Color::Gray)
        .background(theme.colors.context_bg);
    let draft = label(" Draft")
        .foreground(theme.colors.context_light)
        .background(theme.colors.context_bg);

    let open_n = label(&format!("{open}"))
        .foreground(tuirealm::props::Color::Green)
        .background(theme.colors.context_bg);
    let open = label(" Open")
        .foreground(theme.colors.context_light)
        .background(theme.colors.context_bg);

    let archived_n = label(&format!("{archived}"))
        .foreground(tuirealm::props::Color::Yellow)
        .background(theme.colors.context_bg);
    let archived = label(" Archived")
        .foreground(theme.colors.context_light)
        .background(theme.colors.context_bg);

    let merged_n = label(&format!("{merged}"))
        .foreground(tuirealm::props::Color::Cyan)
        .background(theme.colors.context_bg);
    let merged = label(" Merged ")
        .foreground(theme.colors.context_light)
        .background(theme.colors.context_bg);

    let progress = label(&format!(" {} ", progress.to_string()))
        .foreground(theme.colors.context_bg)
        .background(theme.colors.context_badge_bg);

    let spacer = label("").background(theme.colors.context_bg);

    let context_bar = ContextBar::new(
        theme.clone(),
        label_group(&[context]),
        label_group(&[spacer.clone()]),
        label_group(&[spacer]),
        label_group(&[
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
        label_group(&[progress]),
    );

    Widget::new(context_bar).height(1)
}
