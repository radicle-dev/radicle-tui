use radicle::cob::patch::{Patch, PatchId};

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::tui::layout::Rect;
use tuirealm::{AttrValue, Attribute, Frame, MockComponent, Props, State};

use radicle_tui as tui;

use tui::context::Context;
use tui::ui::cob::PatchItem;
use tui::ui::theme::{style, Theme};
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
            tui::ui::label(" ● ").style(style::reset_dim()),
            tui::ui::label("ID").style(style::reset_dim()),
            tui::ui::label("Title").style(style::reset_dim()),
            tui::ui::label("Author").style(style::reset_dim()),
            tui::ui::label("Head").style(style::reset_dim()),
            tui::ui::label("+").style(style::reset_dim()),
            tui::ui::label("-").style(style::reset_dim()),
            tui::ui::label("Updated").style(style::reset_dim()),
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

        let table = Widget::new(Table::new(&items, selected, header, widths, theme.clone()));

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
        vec![tui::ui::reversable_label("Patches").style(style::cyan())],
    )
}

pub fn patches(
    context: &Context,
    theme: &Theme,
    selected: Option<(PatchId, Patch)>,
) -> Widget<PatchBrowser> {
    Widget::new(PatchBrowser::new(context, theme, selected))
}

pub fn browse_context(context: &Context, _theme: &Theme, progress: Progress) -> Widget<ContextBar> {
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

    let context = label(" Patches ").style(style::magenta_reversed());
    let divider = label(" | ").style(style::default_reversed());

    let draft_n = label(&format!("{draft}")).style(style::default_reversed());
    let draft = label(" Draft").style(style::default_reversed());

    let open_n = label(&format!("{open}")).style(style::green_default_reversed());
    let open = label(" Open").style(style::default_reversed());

    let archived_n = label(&format!("{archived}")).style(style::yellow_default_reversed());
    let archived = label(" Archived").style(style::default_reversed());

    let merged_n = label(&format!("{merged}")).style(style::cyan_default_reversed());
    let merged = label(" Merged ").style(style::default_reversed());

    let progress = label(&format!(" {} ", progress.to_string())).style(style::magenta_reversed());
    let spacer = label("").style(style::default_reversed());

    let context_bar = ContextBar::new(
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
