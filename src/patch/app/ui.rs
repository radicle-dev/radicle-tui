use radicle::cob::patch::{Patch, PatchId};

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::tui::layout::Rect;
use tuirealm::{AttrValue, Attribute, Frame, MockComponent, Props, State};

use radicle_tui::ui::context::Context;
use radicle_tui::ui::theme::Theme;
use radicle_tui::ui::widget::common;
use radicle_tui::ui::widget::{Widget, WidgetComponent};
use radicle_tui::ui::{cob, layout};

use common::container::Tabs;
use common::context::{ContextBar, Progress};
use common::label::Label;

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
    common::tabs(
        theme,
        vec![common::reversable_label("patches").foreground(theme.colors.tabs_highlighted_fg)],
    )
}

pub fn navigation(theme: &Theme) -> Widget<Tabs> {
    common::tabs(
        theme,
        vec![
            common::reversable_label("activity").foreground(theme.colors.tabs_highlighted_fg),
            common::reversable_label("files").foreground(theme.colors.tabs_highlighted_fg),
        ],
    )
}

pub fn activity(theme: &Theme) -> Widget<Activity> {
    let not_implemented = common::label("not implemented").foreground(theme.colors.default_fg);
    let activity = Activity::new(not_implemented);

    Widget::new(activity)
}

pub fn files(theme: &Theme) -> Widget<Files> {
    let not_implemented = common::label("not implemented").foreground(theme.colors.default_fg);
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

    common::context::bar(theme, "Patch", &id, title, &author, &comments.to_string())
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

    common::context::bar(
        theme,
        "Browse",
        "",
        "",
        &format!("{draft} draft | {open} open | {archived} archived | {merged} merged"),
        &progress.to_string(),
    )
}
