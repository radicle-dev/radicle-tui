use radicle::cob::patch::{Patch, PatchId};

use tui::ui::widget::context::{ContextBar, Progress};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::tui::layout::Rect;
use tuirealm::{AttrValue, Attribute, Frame, MockComponent, Props, State};

use radicle_tui as tui;

use tui::cob::patch::Filter;
use tui::context::Context;
use tui::ui::cob::PatchItem;
use tui::ui::theme::{style, Theme};
use tui::ui::widget::{Widget, WidgetComponent};

use tui::ui::widget::label::{self};
use tui::ui::widget::list::{ColumnWidth, Table};

pub struct PatchBrowser {
    items: Vec<PatchItem>,
    table: Widget<Table<PatchItem, 8>>,
}

impl PatchBrowser {
    pub fn new(
        theme: &Theme,
        context: &Context,
        filter: Filter,
        selected: Option<(PatchId, Patch)>,
    ) -> Self {
        let header = [
            label::header(" ● "),
            label::header("ID"),
            label::header("Title"),
            label::header("Author"),
            label::header("Head"),
            label::header("+"),
            label::header("-"),
            label::header("Updated"),
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
        let patches = context
            .patches()
            .as_ref()
            .unwrap()
            .iter()
            .filter(|(_, patch)| filter.matches(context.profile(), patch));

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

pub fn browse_context(
    context: &Context,
    _theme: &Theme,
    filter: Filter,
    progress: Progress,
) -> Widget<ContextBar> {
    use radicle::cob::patch::State;

    let mut draft = 0;
    let mut open = 0;
    let mut archived = 0;
    let mut merged = 0;

    let patches = context
        .patches()
        .as_ref()
        .unwrap()
        .iter()
        .filter(|(_, patch)| filter.matches(context.profile(), patch));

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

    let context = label::reversable("/").style(style::magenta_reversed());
    let filter = label::default(&filter.to_string()).style(style::magenta_dim());

    let draft_n = label::default(&format!("{draft}")).style(style::gray_dim());
    let draft = label::default(" Draft");

    let open_n = label::default(&format!("{open}")).style(style::green());
    let open = label::default(" Open");

    let archived_n = label::default(&format!("{archived}")).style(style::yellow());
    let archived = label::default(" Archived");

    let merged_n = label::default(&format!("{merged}")).style(style::cyan());
    let merged = label::default(" Merged ");

    let progress = label::reversable(&progress.to_string()).style(style::magenta_reversed());

    let spacer = label::default("");
    let divider = label::default(" | ");

    let context_bar = ContextBar::new(
        label::group(&[context]),
        label::group(&[filter]),
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
