use radicle::node::AliasStore;

use radicle::cob::thread::Comment;
use radicle::cob::thread::CommentId;

use radicle::cob::issue::Issue;
use radicle::cob::issue::IssueId;

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::tui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::{AttrValue, Attribute, Frame, MockComponent, Props, State};

use radicle_tui as tui;

use tui::common::context::Context;
use tui::realm::ui::cob;
use tui::realm::ui::cob::IssueItem;
use tui::realm::ui::theme::style;
use tui::realm::ui::theme::Theme;
use tui::realm::ui::widget::container::{Container, Tabs};
use tui::realm::ui::widget::context::{ContextBar, Progress};
use tui::realm::ui::widget::form::{Form, TextArea, TextField};
use tui::realm::ui::widget::label::{self, Textarea};
use tui::realm::ui::widget::list::{ColumnWidth, List, Property, Table};
use tui::realm::ui::widget::{Widget, WidgetComponent};

pub const FORM_ID_EDIT: &str = "edit-form";

pub struct IssueBrowser {
    items: Vec<IssueItem>,
    table: Widget<Table<IssueItem, 7>>,
}

impl IssueBrowser {
    pub fn new(context: &Context, theme: &Theme, selected: Option<(IssueId, Issue)>) -> Self {
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
        let mut items = vec![];

        let issues = context.issues().as_ref().unwrap();
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

pub struct LargeList {
    items: Vec<IssueItem>,
    list: Widget<Container>,
}

impl LargeList {
    pub fn new(context: &Context, theme: &Theme, selected: Option<(IssueId, Issue)>) -> Self {
        let repo = context.repository();

        let issues = context.issues().as_ref().unwrap();
        let mut items = issues
            .iter()
            .map(|(id, issue)| IssueItem::from((context.profile(), repo, *id, issue.clone())))
            .collect::<Vec<_>>();

        items.sort_by(|a, b| b.timestamp().cmp(a.timestamp()));
        items.sort_by(|a, b| b.state().cmp(a.state()));

        let selected =
            selected.map(|(id, issue)| IssueItem::from((context.profile(), repo, id, issue)));

        let list = Widget::new(List::new(&items, selected, theme.clone()));

        let container = tui::realm::ui::container(theme, list.to_boxed());

        Self {
            items,
            list: container,
        }
    }

    pub fn items(&self) -> &Vec<IssueItem> {
        &self.items
    }
}

impl WidgetComponent for LargeList {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        self.list.attr(Attribute::Focus, AttrValue::Flag(focus));
        self.list.view(frame, area);
    }

    fn state(&self) -> State {
        self.list.state()
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        self.list.perform(cmd)
    }
}

pub struct IssueHeader {
    container: Widget<Container>,
}

impl IssueHeader {
    pub fn new(context: &Context, theme: &Theme, issue: (IssueId, Issue)) -> Self {
        let repo = context.repository();

        let (id, issue) = issue;
        let author = issue.author();
        let author = author.id();
        let alias = context.profile().aliases().alias(author);
        let by_you = *author == context.profile().did();
        let item = IssueItem::from((context.profile(), repo, id, issue.clone()));

        let title = Property::new(label::property("Title"), label::default(item.title()));

        let author = match alias {
            Some(_) => label::alias(&cob::format_author(issue.author().id(), &alias, by_you)),
            None => label::did(&cob::format_author(issue.author().id(), &alias, by_you)),
        };
        let author = Property::new(label::property("Author"), author);

        let issue_id = Property::new(
            label::property("Issue"),
            label::default(&id.to_string()).style(style::gray()),
        );

        let labels = Property::new(
            label::property("Labels"),
            label::labels(&cob::format_labels(item.labels())),
        );

        let assignees = Property::new(
            label::property("Assignees"),
            label::did(&cob::format_assignees(
                &item
                    .assignees()
                    .iter()
                    .map(|item| (item.did(), item.alias(), item.is_you()))
                    .collect::<Vec<_>>(),
            )),
        );

        let state = Property::new(
            label::property("Status"),
            label::default(&item.state().to_string()),
        );

        let table = tui::realm::ui::property_table(
            theme,
            vec![
                Widget::new(title),
                Widget::new(issue_id),
                Widget::new(author),
                Widget::new(labels),
                Widget::new(assignees),
                Widget::new(state),
            ],
        );
        let container = tui::realm::ui::container(theme, table.to_boxed());

        Self { container }
    }
}

impl WidgetComponent for IssueHeader {
    fn view(&mut self, _properties: &Props, frame: &mut Frame, area: Rect) {
        self.container.view(frame, area);
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

pub struct IssueDetails {
    header: Widget<IssueHeader>,
    description: Widget<CommentBody>,
}

impl IssueDetails {
    pub fn new(
        context: &Context,
        theme: &Theme,
        issue: (IssueId, Issue),
        description: Option<(&CommentId, &Comment)>,
    ) -> Self {
        Self {
            header: header(context, theme, issue),
            description: self::description(context, theme, description),
        }
    }
}

impl WidgetComponent for IssueDetails {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(8), Constraint::Min(1)])
            .split(area);

        self.header.view(frame, layout[0]);

        self.description
            .attr(Attribute::Focus, AttrValue::Flag(focus));
        self.description.view(frame, layout[1]);
    }

    fn state(&self) -> State {
        self.description.state()
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        self.description.perform(cmd)
    }
}

pub struct CommentBody {
    textarea: Widget<Container>,
}

impl CommentBody {
    pub fn new(_context: &Context, theme: &Theme, comment: Option<(&CommentId, &Comment)>) -> Self {
        let content = match comment {
            Some((_, comment)) => comment.body().to_string(),
            None => String::new(),
        };
        let textarea = Widget::new(Textarea::default())
            .content(AttrValue::String(content))
            .style(style::reset());

        let textarea = tui::realm::ui::container(theme, textarea.to_boxed());

        Self { textarea }
    }
}

impl WidgetComponent for CommentBody {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        self.textarea.attr(Attribute::Focus, AttrValue::Flag(focus));
        self.textarea.view(frame, area);
    }

    fn state(&self) -> State {
        self.textarea.state()
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        self.textarea.perform(cmd)
    }
}

pub fn list_navigation(theme: &Theme) -> Widget<Tabs> {
    tui::realm::ui::tabs(
        theme,
        vec![label::reversable("Issues").style(style::magenta())],
    )
}

pub fn list(
    context: &Context,
    theme: &Theme,
    issue: Option<(IssueId, Issue)>,
) -> Widget<LargeList> {
    let list = LargeList::new(context, theme, issue);

    Widget::new(list)
}

pub fn header(context: &Context, theme: &Theme, issue: (IssueId, Issue)) -> Widget<IssueHeader> {
    let header = IssueHeader::new(context, theme, issue);
    Widget::new(header)
}

pub fn description(
    context: &Context,
    theme: &Theme,
    comment: Option<(&CommentId, &Comment)>,
) -> Widget<CommentBody> {
    let body = CommentBody::new(context, theme, comment);
    Widget::new(body)
}

pub fn new_form(_context: &Context, theme: &Theme) -> Widget<Form> {
    use tuirealm::props::Layout;

    let title = Widget::new(TextField::new(theme.clone(), "Title")).to_boxed();
    let tags = Widget::new(TextField::new(theme.clone(), "Labels (bug, ...)")).to_boxed();
    let assignees = Widget::new(TextField::new(
        theme.clone(),
        "Assignees (z6MkvAdxCp1oLVVTsqYvev9YrhSN3gBQNUSM45hhy4pgkexk, ...)",
    ))
    .to_boxed();
    let description = Widget::new(TextArea::new(theme.clone(), "Description")).to_boxed();
    let inputs: Vec<Box<dyn MockComponent>> = vec![title, tags, assignees, description];

    let layout = Layout::default().constraints(
        [
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
        ]
        .as_ref(),
    );

    Widget::new(Form::new(theme.clone(), inputs))
        .custom(Form::PROP_ID, AttrValue::String(String::from(FORM_ID_EDIT)))
        .layout(layout)
}

pub fn details(
    context: &Context,
    theme: &Theme,
    issue: (IssueId, Issue),
    comment: Option<(&CommentId, &Comment)>,
) -> Widget<IssueDetails> {
    let discussion = IssueDetails::new(context, theme, issue, comment);
    Widget::new(discussion)
}

pub fn browse_context(context: &Context, theme: &Theme, progress: Progress) -> Widget<ContextBar> {
    use radicle::cob::issue::State;

    let issues = context.issues().as_ref().unwrap();
    let open = issues
        .iter()
        .filter(|issue| *issue.1.state() == State::Open)
        .collect::<Vec<_>>()
        .len();
    let closed = issues
        .iter()
        .filter(|issue| *issue.1.state() != State::Open)
        .collect::<Vec<_>>()
        .len();

    tui::realm::ui::widget::context::bar(
        theme,
        "Browse",
        "",
        "",
        &format!("{open} open | {closed} closed"),
        &progress.to_string(),
    )
}

pub fn description_context(
    _context: &Context,
    theme: &Theme,
    progress: Progress,
) -> Widget<ContextBar> {
    tui::realm::ui::widget::context::bar(theme, "Show", "", "", "", &progress.to_string())
}

pub fn form_context(_context: &Context, theme: &Theme, progress: Progress) -> Widget<ContextBar> {
    tui::realm::ui::widget::context::bar(theme, "Open", "", "", "", &progress.to_string())
        .custom(ContextBar::PROP_EDIT_MODE, AttrValue::Flag(true))
}

pub fn issues(
    context: &Context,
    theme: &Theme,
    selected: Option<(IssueId, Issue)>,
) -> Widget<IssueBrowser> {
    Widget::new(IssueBrowser::new(context, theme, selected))
}
