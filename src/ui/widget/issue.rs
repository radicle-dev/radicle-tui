use radicle::cob::thread::Comment;
use radicle::cob::thread::CommentId;

use radicle::cob::issue::Issue;
use radicle::cob::issue::IssueId;
use tuirealm::tui::layout::Constraint;
use tuirealm::tui::layout::Direction;
use tuirealm::tui::layout::Layout;

use super::common::container::Container;
use super::common::container::LabeledContainer;
use super::common::context::ContextBar;
use super::common::context::Progress;
use super::common::label::Textarea;
use super::common::list::List;
use super::common::list::Property;
use super::Widget;

use crate::ui::cob;
use crate::ui::cob::IssueItem;
use crate::ui::context::Context;
use crate::ui::theme::Theme;

use super::*;

pub struct LargeList {
    items: Vec<IssueItem>,
    list: Widget<LabeledContainer>,
}

impl LargeList {
    pub fn new(context: &Context, theme: &Theme, selected: Option<(IssueId, Issue)>) -> Self {
        let repo = context.repository();
        let issues = crate::cob::issue::all(repo).unwrap_or_default();
        let mut items = issues
            .iter()
            .map(|(id, issue)| IssueItem::from((context.profile(), repo, *id, issue.clone())))
            .collect::<Vec<_>>();

        items.sort_by(|a, b| b.timestamp().cmp(a.timestamp()));
        items.sort_by(|a, b| b.state().cmp(a.state()));

        let selected =
            selected.map(|(id, issue)| IssueItem::from((context.profile(), repo, id, issue)));

        let list = Widget::new(List::new(&items, selected, theme.clone()))
            .highlight(theme.colors.item_list_highlighted_bg);

        let container = common::labeled_container(theme, "Issues", list.to_boxed());

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
            common::label("Title").foreground(theme.colors.property_name_fg),
            common::label(item.title()).foreground(theme.colors.browser_list_title),
        );

        let author = Property::new(
            common::label("Author").foreground(theme.colors.property_name_fg),
            common::label(&cob::format_author(issue.author().id(), by_you))
                .foreground(theme.colors.browser_list_author),
        );

        let issue_id = Property::new(
            common::label("Issue").foreground(theme.colors.property_name_fg),
            common::label(&id.to_string()).foreground(theme.colors.browser_list_description),
        );

        let tags = Property::new(
            common::label("Tags").foreground(theme.colors.property_name_fg),
            common::label(&cob::format_tags(item.tags()))
                .foreground(theme.colors.browser_list_tags),
        );

        let assignees = Property::new(
            common::label("Assignees").foreground(theme.colors.property_name_fg),
            common::label(&cob::format_assignees(
                &item
                    .assignees()
                    .iter()
                    .map(|item| (item.did(), item.is_you()))
                    .collect::<Vec<_>>(),
            ))
            .foreground(theme.colors.browser_list_author),
        );

        let state = Property::new(
            common::label("Status").foreground(theme.colors.property_name_fg),
            common::label(&item.state().to_string()).foreground(theme.colors.browser_list_title),
        );

        let table = common::property_table(
            theme,
            vec![
                Widget::new(title),
                Widget::new(issue_id),
                Widget::new(author),
                Widget::new(tags),
                Widget::new(assignees),
                Widget::new(state),
            ],
        );
        let container = common::container(theme, table.to_boxed());

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
            description: issue::description(context, theme, description),
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

        let textarea = common::container(theme, textarea.to_boxed());

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

pub fn list(context: &Context, theme: &Theme, issue: (IssueId, Issue)) -> Widget<LargeList> {
    let list = LargeList::new(context, theme, Some(issue));

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

    common::context::bar(
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
    common::context::bar(theme, "Show", "", "", "", &progress.to_string())
}
