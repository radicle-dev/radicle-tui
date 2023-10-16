use radicle::cob::thread::Comment;
use radicle::cob::thread::CommentId;

use radicle::cob::issue::Issue;
use radicle::cob::issue::IssueId;

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::tui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::{AttrValue, Attribute, Frame, MockComponent, Props, State};

use radicle_tui as tui;

use tui::context::Context;
use tui::ui::cob;
use tui::ui::cob::IssueItem;
use tui::ui::theme::Theme;
use tui::ui::widget::{Widget, WidgetComponent};

use tui::ui::widget::container::{Container, Tabs};
use tui::ui::widget::context::{ContextBar, Progress};
use tui::ui::widget::form::Form;
use tui::ui::widget::label::Textarea;
use tui::ui::widget::list::{ColumnWidth, List, Property, Table};

pub struct IssueBrowser {
    items: Vec<IssueItem>,
    table: Widget<Table<IssueItem, 7>>,
}

impl IssueBrowser {
    pub fn new(context: &Context, theme: &Theme, selected: Option<(IssueId, Issue)>) -> Self {
        let header = [
            tui::ui::label(" ● "),
            tui::ui::label("ID"),
            tui::ui::label("Title"),
            tui::ui::label("Author"),
            tui::ui::label("Tags"),
            tui::ui::label("Assignees"),
            tui::ui::label("Opened"),
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

        let selected = match selected {
            Some((id, issue)) => Some(IssueItem::from((context.profile(), repo, id, issue))),
            _ => items.first().cloned(),
        };

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

pub struct LargeList {
    items: Vec<IssueItem>,
    list: Widget<Container>,
}

impl LargeList {
    pub fn new(context: &Context, theme: &Theme, selected: Option<(IssueId, Issue)>) -> Self {
        let repo = context.repository();

        let mut items = context
            .issues()
            .iter()
            .map(|(id, issue)| IssueItem::from((context.profile(), repo, *id, issue.clone())))
            .collect::<Vec<_>>();

        items.sort_by(|a, b| b.timestamp().cmp(a.timestamp()));
        items.sort_by(|a, b| b.state().cmp(a.state()));

        let selected =
            selected.map(|(id, issue)| IssueItem::from((context.profile(), repo, id, issue)));

        let list = Widget::new(List::new(&items, selected, theme.clone()))
            .highlight(theme.colors.item_list_highlighted_bg);

        let container = tui::ui::container(theme, list.to_boxed());

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
        let by_you = *issue.author().id() == context.profile().did();
        let item = IssueItem::from((context.profile(), repo, id, issue.clone()));

        let title = Property::new(
            tui::ui::label("Title").foreground(theme.colors.property_name_fg),
            tui::ui::label(item.title()).foreground(theme.colors.browser_list_title),
        );

        let author = Property::new(
            tui::ui::label("Author").foreground(theme.colors.property_name_fg),
            tui::ui::label(&cob::format_author(issue.author().id(), by_you))
                .foreground(theme.colors.browser_list_author),
        );

        let issue_id = Property::new(
            tui::ui::label("Issue").foreground(theme.colors.property_name_fg),
            tui::ui::label(&id.to_string()).foreground(theme.colors.browser_list_description),
        );

        let labels = Property::new(
            tui::ui::label("Labels").foreground(theme.colors.property_name_fg),
            tui::ui::label(&cob::format_labels(item.labels()))
                .foreground(theme.colors.browser_list_labels),
        );

        let assignees = Property::new(
            tui::ui::label("Assignees").foreground(theme.colors.property_name_fg),
            tui::ui::label(&cob::format_assignees(
                &item
                    .assignees()
                    .iter()
                    .map(|item| (item.did(), item.is_you()))
                    .collect::<Vec<_>>(),
            ))
            .foreground(theme.colors.browser_list_author),
        );

        let state = Property::new(
            tui::ui::label("Status").foreground(theme.colors.property_name_fg),
            tui::ui::label(&item.state().to_string()).foreground(theme.colors.browser_list_title),
        );

        let table = tui::ui::property_table(
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
        let container = tui::ui::container(theme, table.to_boxed());

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
        let textarea = Widget::new(Textarea::new(theme.clone()))
            .content(AttrValue::String(content))
            .foreground(theme.colors.default_fg);

        let textarea = tui::ui::container(theme, textarea.to_boxed());

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

pub struct OpenForm {
    form: Widget<Form>,
}

impl OpenForm {
    pub fn new(form: Widget<Form>) -> Self {
        Self { form }
    }
}

impl WidgetComponent for OpenForm {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        self.form.attr(Attribute::Focus, AttrValue::Flag(focus));
        self.form.view(frame, area);
    }

    fn state(&self) -> State {
        self.form.state()
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        self.form.perform(cmd)
    }
}

pub struct EditForm {
    form: Widget<Form>,
}

impl EditForm {
    pub fn new(form: Widget<Form>) -> Self {
        Self { form }
    }
}

impl WidgetComponent for EditForm {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        self.form.attr(Attribute::Focus, AttrValue::Flag(focus));
        self.form.view(frame, area);
    }

    fn state(&self) -> State {
        self.form.state()
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        self.form.perform(cmd)
    }
}

pub fn list_navigation(theme: &Theme) -> Widget<Tabs> {
    tui::ui::tabs(
        theme,
        vec![tui::ui::reversable_label("Issues").foreground(theme.colors.tabs_highlighted_fg)],
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

pub fn open_form(_context: &Context, theme: &Theme) -> Widget<OpenForm> {
    use tuirealm::props::Layout;

    let title = form::title(theme, None).to_boxed();
    let labels = form::labels(theme, None).to_boxed();
    let assignees = form::assignees(theme, None).to_boxed();
    let description = form::description(theme, None).to_boxed();

    let inputs: Vec<Box<dyn MockComponent>> = vec![title, labels, assignees, description];
    let layout = Layout::default().constraints(
        [
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
        ]
        .as_ref(),
    );

    let form = Widget::new(Form::new(theme.clone(), inputs, vec![])).layout(layout);
    Widget::new(OpenForm::new(form))
}

pub fn edit_form(context: &Context, theme: &Theme, issue: (IssueId, Issue)) -> Widget<EditForm> {
    use tuirealm::props::Layout;

    let repo = context.repository();
    let (id, issue) = issue;
    let item = IssueItem::from((context.profile(), repo, id, issue));

    let hidden_id = &item.id().to_string();
    let title = item.title();
    let labels = &cob::format_labels(item.labels());
    let assignees = &cob::format_assignees(
        &item
            .assignees()
            .iter()
            .map(|item| (item.did(), false))
            .collect::<Vec<_>>(),
    );
    let description = item.description();

    let hidden_id = form::hidden(theme, Some(hidden_id)).to_boxed();
    let title = form::title(theme, Some(title)).to_boxed();
    let labels = form::labels(theme, Some(labels)).to_boxed();
    let assignees = form::assignees(theme, Some(assignees)).to_boxed();
    let description = form::description(theme, Some(description)).to_boxed();
    let state = form::state(theme, item.state()).to_boxed();

    let inputs: Vec<Box<dyn MockComponent>> = vec![title, labels, assignees, description, state];

    let layout = Layout::default().constraints(
        [
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(3),
        ]
        .as_ref(),
    );

    let form = Widget::new(Form::new(theme.clone(), inputs, vec![hidden_id])).layout(layout);
    Widget::new(EditForm::new(form))
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

    let issues = context.issues();
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

    tui::ui::widget::context::bar(
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
    tui::ui::widget::context::bar(theme, "Show", "", "", "", &progress.to_string())
}

pub fn form_context(_context: &Context, theme: &Theme, progress: Progress) -> Widget<ContextBar> {
    tui::ui::widget::context::bar(theme, "Open", "", "", "", &progress.to_string())
        .custom(ContextBar::PROP_EDIT_MODE, AttrValue::Flag(true))
}

pub fn issues(
    context: &Context,
    theme: &Theme,
    selected: Option<(IssueId, Issue)>,
) -> Widget<IssueBrowser> {
    Widget::new(IssueBrowser::new(context, theme, selected))
}

mod form {
    use radicle::cob::issue::State;
    use radicle_tui as tui;

    use tui::ui::theme::Theme;
    use tui::ui::widget::form::TextField;
    use tui::ui::widget::form::*;
    use tui::ui::widget::Widget;

    pub fn hidden(theme: &Theme, value: Option<&str>) -> Widget<TextField> {
        hidden_field(theme, value)
    }

    pub fn title(theme: &Theme, value: Option<&str>) -> Widget<TextField> {
        text_field(theme, "Title", "Title", value)
    }

    pub fn labels(theme: &Theme, value: Option<&str>) -> Widget<TextField> {
        text_field(theme, "Labels", "Labels (bug, ...)", value)
    }

    pub fn assignees(theme: &Theme, value: Option<&str>) -> Widget<TextField> {
        text_field(
            theme,
            "Assignees",
            "Assignees (z6MkvAdxCp1oLVVTsqYvev9YrhSN3gBQNUSM45hhy4pgkexk, ...)",
            value,
        )
    }

    pub fn description(theme: &Theme, value: Option<&str>) -> Widget<TextArea> {
        text_area(theme, "Description", "Description", value)
    }

    pub fn state(theme: &Theme, state: &State) -> Widget<Radio> {
        let choices = &[
            "Open".to_owned(),
            "Closed as solved".to_owned(),
            "Closed as other".to_owned(),
        ];
        let selected = tui::cob::state::to_u16(state);

        radio(theme, "State", choices, selected)
    }
}
