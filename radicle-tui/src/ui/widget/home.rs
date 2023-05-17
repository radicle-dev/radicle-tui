use radicle::prelude::{Id, Project};
use radicle::storage::ReadStorage;
use radicle::Profile;

use radicle::cob::patch::Patches;

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::tui::layout::Rect;
use tuirealm::{AttrValue, Attribute, Frame, MockComponent, Props, State};

use super::common;
use super::common::container::{LabeledContainer, Tabs};
use super::common::context::Shortcuts;
use super::common::label::Label;
use super::common::list::{ColumnWidth, Table};

use super::{Widget, WidgetComponent};

use crate::ui::cob::PatchItem;
use crate::ui::layout;
use crate::ui::theme::Theme;

pub struct Dashboard {
    about: Widget<LabeledContainer>,
    shortcuts: Widget<Shortcuts>,
}

impl Dashboard {
    pub fn new(about: Widget<LabeledContainer>, shortcuts: Widget<Shortcuts>) -> Self {
        Self { about, shortcuts }
    }
}

impl WidgetComponent for Dashboard {
    fn view(&mut self, _properties: &Props, frame: &mut Frame, area: Rect) {
        let shortcuts_h = self
            .shortcuts
            .query(Attribute::Height)
            .unwrap_or(AttrValue::Size(0))
            .unwrap_size();
        let layout = layout::root_component(area, shortcuts_h);

        self.about.view(frame, layout[0]);
        self.shortcuts.view(frame, layout[1]);
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

pub struct IssueBrowser {
    label: Widget<Label>,
    shortcuts: Widget<Shortcuts>,
}

impl IssueBrowser {
    pub fn new(label: Widget<Label>, shortcuts: Widget<Shortcuts>) -> Self {
        Self { label, shortcuts }
    }
}

impl WidgetComponent for IssueBrowser {
    fn view(&mut self, _properties: &Props, frame: &mut Frame, area: Rect) {
        let label_w = self
            .label
            .query(Attribute::Width)
            .unwrap_or(AttrValue::Size(1))
            .unwrap_size();
        let shortcuts_h = self
            .shortcuts
            .query(Attribute::Height)
            .unwrap_or(AttrValue::Size(0))
            .unwrap_size();
        let layout = layout::root_component(area, shortcuts_h);

        self.label
            .view(frame, layout::centered_label(label_w, layout[0]));
        self.shortcuts.view(frame, layout[1])
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

pub struct PatchBrowser {
    table: Widget<Table<PatchItem, 8>>,
    shortcuts: Widget<Shortcuts>,
}

impl PatchBrowser {
    pub fn new(profile: &Profile, id: &Id, shortcuts: Widget<Shortcuts>, theme: Theme) -> Self {
        let repo = profile.storage.repository(*id).unwrap();
        let patches = Patches::open(&repo)
            .and_then(|patches| patches.all().map(|iter| iter.flatten().collect::<Vec<_>>()));

        let header = [
            common::label(" ● "),
            common::label("ID"),
            common::label("Title"),
            common::label("Author"),
            common::label("Head"),
            common::label("+"),
            common::label("-"),
            common::label("Updated"),
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

        let mut items = vec![];
        if let Ok(mut patches) = patches {
            patches.sort_by(|(_, a, _), (_, b, _)| b.timestamp().cmp(&a.timestamp()));
            patches.sort_by(|(_, a, _), (_, b, _)| a.state().cmp(b.state()));

            for (id, patch, _) in patches {
                if let Ok(item) = PatchItem::try_from((profile, &repo, id, patch)) {
                    items.push(item);
                }
            }
        }

        let table = Widget::new(Table::new(&items, header, widths, theme.clone()))
            .highlight(theme.colors.item_list_highlighted_bg);

        Self { table, shortcuts }
    }

    pub fn selected_item(&self) -> Option<&PatchItem> {
        self.table.selection()
    }
}

impl WidgetComponent for PatchBrowser {
    fn view(&mut self, _properties: &Props, frame: &mut Frame, area: Rect) {
        let shortcuts_h = self
            .shortcuts
            .query(Attribute::Height)
            .unwrap_or(AttrValue::Size(0))
            .unwrap_size();
        let layout = layout::root_component(area, shortcuts_h);

        self.table.view(frame, layout[0]);
        self.shortcuts.view(frame, layout[1]);
    }

    fn state(&self) -> State {
        self.table.state()
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        self.table.perform(cmd)
    }
}

pub fn navigation(theme: &Theme) -> Widget<Tabs> {
    common::tabs(
        theme,
        vec![
            common::reversable_label("dashboard").foreground(theme.colors.tabs_highlighted_fg),
            common::reversable_label("issues").foreground(theme.colors.tabs_highlighted_fg),
            common::reversable_label("patches").foreground(theme.colors.tabs_highlighted_fg),
        ],
    )
}

pub fn dashboard(theme: &Theme, id: &Id, project: &Project) -> Widget<Dashboard> {
    let about = common::labeled_container(
        theme,
        "about",
        common::property_list(
            theme,
            vec![
                common::property(theme, "id", &id.to_string()),
                common::property(theme, "name", project.name()),
                common::property(theme, "description", project.description()),
            ],
        )
        .to_boxed(),
    );
    let shortcuts = common::shortcuts(
        theme,
        vec![
            common::shortcut(theme, "tab", "section"),
            common::shortcut(theme, "q", "quit"),
        ],
    );
    let dashboard = Dashboard::new(about, shortcuts);

    Widget::new(dashboard)
}

pub fn patches(theme: &Theme, id: &Id, profile: &Profile) -> Widget<PatchBrowser> {
    let shortcuts = common::shortcuts(
        theme,
        vec![
            common::shortcut(theme, "tab", "section"),
            common::shortcut(theme, "↑/↓", "navigate"),
            common::shortcut(theme, "enter", "show"),
            common::shortcut(theme, "q", "quit"),
        ],
    );

    Widget::new(PatchBrowser::new(profile, id, shortcuts, theme.clone()))
}

pub fn issues(theme: &Theme) -> Widget<IssueBrowser> {
    let shortcuts = common::shortcuts(
        theme,
        vec![
            common::shortcut(theme, "tab", "section"),
            common::shortcut(theme, "q", "quit"),
        ],
    );

    let not_implemented = common::label("not implemented").foreground(theme.colors.default_fg);
    let browser = IssueBrowser::new(not_implemented, shortcuts);

    Widget::new(browser)
}
