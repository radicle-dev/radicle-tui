use radicle::issue::{Issue, IssueId};

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::tui::layout::Rect;
use tuirealm::{AttrValue, Attribute, Frame, MockComponent, Props, State};

use radicle_tui as tui;

use tui::cob::issue::Filter;
use tui::context::Context;
use tui::ui::cob::IssueItem;
use tui::ui::theme::{style, Theme};
use tui::ui::widget::context::{ContextBar, Progress};
use tui::ui::widget::{Widget, WidgetComponent};

use tui::ui::widget::label::{self};
use tui::ui::widget::list::{ColumnWidth, Table};

pub struct IssueBrowser {
    items: Vec<IssueItem>,
    table: Widget<Table<IssueItem, 7>>,
}

impl IssueBrowser {
    pub fn new(
        theme: &Theme,
        context: &Context,
        filter: Filter,
        selected: Option<(IssueId, Issue)>,
    ) -> Self {
        let header = [
            label::header(" ● "),
            label::header("ID"),
            label::header("Title"),
            label::header("Author"),
            label::header("Tags"),
            label::header("Assignees"),
            label::header("Opened"),
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
        let issues = context
            .issues()
            .as_ref()
            .unwrap()
            .iter()
            .filter(|(_, issue)| filter.matches(context.profile(), issue));

        let mut items = vec![];
        for (id, issue) in issues {
            if let Ok(item) = IssueItem::try_from((context.profile(), repo, *id, issue.clone())) {
                items.push(item);
            }
        }

        items.sort_by(|a, b| b.timestamp().cmp(a.timestamp()));
        items.sort_by(|a, b| b.state().cmp(a.state()));

        let selected = match selected {
            Some((id, issue)) => Some(IssueItem::from((context.profile(), repo, id, issue))),
            _ => items.first().cloned(),
        };

        let table = Widget::new(Table::new(&items, selected, header, widths, theme.clone()));

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

pub fn browse_context(
    context: &Context,
    _theme: &Theme,
    filter: Filter,
    progress: Progress,
) -> Widget<ContextBar> {
    use radicle::issue::State;

    let mut open = 0;
    let mut closed = 0;

    let issues = context
        .issues()
        .as_ref()
        .unwrap()
        .iter()
        .filter(|(_, issue)| filter.matches(context.profile(), issue));

    for (_, issue) in issues {
        match issue.state() {
            State::Open => open += 1,
            State::Closed { reason: _ } => closed += 1,
        }
    }

    let context = label::reversable("/").style(style::magenta_reversed());
    let filter = label::default(&filter.to_string()).style(style::magenta_dim());

    let open_n = label::default(&format!("{open}")).style(style::green());
    let open = label::default(" Open");

    let closed_n = label::default(&format!("{closed}")).style(style::cyan());
    let closed = label::default(" Closed ");

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
            spacer,
            open_n,
            open,
            divider,
            closed_n,
            closed,
        ]),
        label::group(&[progress]),
    );

    Widget::new(context_bar).height(1)
}
