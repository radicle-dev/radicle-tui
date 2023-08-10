use std::collections::HashMap;

use radicle::cob::thread::Comment;
use radicle::cob::thread::CommentId;

use radicle::cob::issue::Issue;
use radicle::cob::issue::IssueId;
use tuirealm::tui::layout::{Constraint, Direction, Layout};
use tuirealm::StateValue;

use super::common::container::{Container, LabeledContainer};
use super::common::context::{ContextBar, Progress};
use super::common::form::Form;
use super::common::label::Textarea;
use super::common::list::List;
use super::common::list::Property;
use super::Widget;

use crate::ui::cob;
use crate::ui::cob::IssueItem;
use crate::ui::context::Context;
use crate::ui::theme::Theme;
use crate::ui::widget::common::form::TextInput;

use super::*;

pub struct LargeList {
    items: Vec<IssueItem>,
    list: Widget<LabeledContainer>,
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

        let labels = Property::new(
            common::label("Tags").foreground(theme.colors.property_name_fg),
            common::label(&cob::format_labels(item.labels()))
                .foreground(theme.colors.browser_list_labels),
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
                Widget::new(labels),
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

pub struct NewForm {
    /// The issue this form writes its input values to.
    _issue: Issue,
    /// The actual form.
    form: Widget<Form>,
}

impl NewForm {
    pub const INPUT_TITLE: &str = "title";
    pub const INPUT_TAGS: &str = "tags";
    pub const INPUT_ASSIGNESS: &str = "assignees";
    pub const INPUT_DESCRIPTION: &str = "description";

    pub fn new(theme: &Theme) -> Self {
        use tuirealm::props::Layout;

        let title = Widget::new(TextInput::new(theme.clone(), "Title"));
        let tags = Widget::new(TextInput::new(theme.clone(), "Tags (tag1, tag2, ...)"));
        let assignees = Widget::new(TextInput::new(
            theme.clone(),
            "Assignees (z6MkvAdxCp1oLVVTsqYvev9YrhSN3gBQNUSM45hhy4pgkexk, ...)",
        ));
        let description = Widget::new(TextInput::new(theme.clone(), "Description"))
            .custom(TextInput::PROP_MULTILINE, AttrValue::Flag(true));

        let mut form = Widget::new(Form::new(
            theme.clone(),
            vec![title, tags, assignees, description],
        ));

        form.attr(
            Attribute::Layout,
            AttrValue::Layout(
                Layout::default().constraints(
                    [
                        Constraint::Length(3),
                        Constraint::Length(3),
                        Constraint::Length(3),
                        Constraint::Min(3),
                    ]
                    .as_ref(),
                ),
            ),
        );

        Self {
            _issue: Issue::default(),
            form,
        }
    }
}

impl WidgetComponent for NewForm {
    fn view(&mut self, _properties: &Props, frame: &mut Frame, area: Rect) {
        self.form.view(frame, area);
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        match self.form.perform(cmd) {
            CmdResult::Submit(State::Vec(values)) => {
                let inputs = HashMap::from([
                    (
                        Self::INPUT_TITLE.to_owned(),
                        values.get(0).unwrap_or(&StateValue::None).clone(),
                    ),
                    (
                        Self::INPUT_TAGS.to_owned(),
                        values.get(1).unwrap_or(&StateValue::None).clone(),
                    ),
                    (
                        Self::INPUT_ASSIGNESS.to_owned(),
                        values.get(2).unwrap_or(&StateValue::None).clone(),
                    ),
                    (
                        Self::INPUT_DESCRIPTION.to_owned(),
                        values.get(3).unwrap_or(&StateValue::None).clone(),
                    ),
                ]);
                CmdResult::Submit(State::Map(inputs))
            }
            _ => CmdResult::None,
        }
    }
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

pub fn new_form(_context: &Context, theme: &Theme) -> Widget<NewForm> {
    let form = NewForm::new(theme);
    Widget::new(form)
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

pub fn form_context(_context: &Context, theme: &Theme, progress: Progress) -> Widget<ContextBar> {
    common::context::bar(theme, "Edit", "", "", "", &progress.to_string())
        .custom(ContextBar::PROP_EDIT_MODE, AttrValue::Flag(true))
}
