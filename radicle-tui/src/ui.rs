pub mod cob;
pub mod components;
pub mod layout;
pub mod state;
pub mod theme;
pub mod widget;

use radicle::prelude::{Id, Project};
use radicle::Profile;
use tuirealm::props::{AttrValue, Attribute, PropPayload, PropValue, TextSpan};
use tuirealm::MockComponent;

use radicle::cob::patch::{Patch, PatchId};

use components::container::{GlobalListener, LabeledContainer, Tabs};
use components::context::{Shortcut, Shortcuts};
use components::label::Label;
use components::list::{Property, PropertyList};

use widget::Widget;

use self::components::list::{List, Table};
use self::components::workspace::Browser;

pub fn global_listener() -> Widget<GlobalListener> {
    Widget::new(GlobalListener::default())
}

pub fn label(content: &str) -> Widget<Label> {
    // TODO: Remove when size constraints are implemented
    let width = content.chars().count() as u16;

    Widget::new(Label::default())
        .content(AttrValue::String(content.to_string()))
        .height(1)
        .width(width)
}

pub fn labeled_container(
    theme: &theme::Theme,
    title: &str,
    component: Box<dyn MockComponent>,
) -> Widget<LabeledContainer> {
    let title = label(&format!(" {title} "))
        .foreground(theme.colors.default_fg)
        .background(theme.colors.labeled_container_bg);
    let container = LabeledContainer::new(title, component);

    Widget::new(container).background(theme.colors.labeled_container_bg)
}

pub fn shortcut(theme: &theme::Theme, short: &str, long: &str) -> Widget<Shortcut> {
    let short = label(short).foreground(theme.colors.shortcut_short_fg);
    let divider = label(&theme.icons.whitespace.to_string());
    let long = label(long).foreground(theme.colors.shortcut_long_fg);

    // TODO: Remove when size constraints are implemented
    let short_w = short.query(Attribute::Width).unwrap().unwrap_size();
    let divider_w = divider.query(Attribute::Width).unwrap().unwrap_size();
    let long_w = long.query(Attribute::Width).unwrap().unwrap_size();
    let width = short_w.saturating_add(divider_w).saturating_add(long_w);

    let shortcut = Shortcut::new(short, divider, long);

    Widget::new(shortcut).height(1).width(width)
}

pub fn shortcuts(theme: &theme::Theme, shortcuts: Vec<Widget<Shortcut>>) -> Widget<Shortcuts> {
    let divider = label(&format!(" {} ", theme.icons.shortcutbar_divider))
        .foreground(theme.colors.shortcutbar_divider_fg);
    let shortcut_bar = Shortcuts::new(shortcuts, divider);

    Widget::new(shortcut_bar).height(1)
}

pub fn property(theme: &theme::Theme, name: &str, value: &str) -> Widget<Property> {
    let name = label(name).foreground(theme.colors.property_name_fg);
    let divider = label(&format!(" {} ", theme.icons.property_divider));
    let value = label(value).foreground(theme.colors.default_fg);

    // TODO: Remove when size constraints are implemented
    let name_w = name.query(Attribute::Width).unwrap().unwrap_size();
    let divider_w = divider.query(Attribute::Width).unwrap().unwrap_size();
    let value_w = value.query(Attribute::Width).unwrap().unwrap_size();
    let width = name_w.saturating_add(divider_w).saturating_add(value_w);

    let property = Property::new(name, divider, value);

    Widget::new(property).height(1).width(width)
}

pub fn property_list(
    _theme: &theme::Theme,
    properties: Vec<Widget<Property>>,
) -> Widget<PropertyList> {
    let property_list = PropertyList::new(properties);

    Widget::new(property_list)
}

pub fn tabs(theme: &theme::Theme, tabs: Vec<Widget<Label>>) -> Widget<Tabs> {
    let divider = label(&theme.icons.tab_divider.to_string());
    let tabs = Tabs::new(tabs, divider);

    Widget::new(tabs)
        .height(1)
        .foreground(theme.colors.tabs_fg)
        .highlight(theme.colors.tabs_highlighted_fg)
}

pub fn table(theme: &theme::Theme, items: &[impl List], profile: &Profile) -> Widget<Table> {
    let table = Table::default();
    let items = items.iter().map(|item| item.row(theme, profile)).collect();

    Widget::new(table)
        .content(AttrValue::Table(items))
        .background(theme.colors.labeled_container_bg)
        .highlight(theme.colors.item_list_highlighted_bg)
}

pub fn patch_browser(
    theme: &theme::Theme,
    items: &[(PatchId, Patch)],
    profile: &Profile,
) -> Widget<Browser<(PatchId, Patch)>> {
    let widths = AttrValue::Payload(PropPayload::Vec(vec![
        PropValue::U16(2),
        PropValue::U16(43),
        PropValue::U16(15),
        PropValue::U16(15),
        PropValue::U16(5),
        PropValue::U16(20),
    ]));
    let header = AttrValue::Payload(PropPayload::Vec(vec![
        PropValue::TextSpan(TextSpan::from("")),
        PropValue::TextSpan(TextSpan::from("title")),
        PropValue::TextSpan(TextSpan::from("author")),
        PropValue::TextSpan(TextSpan::from("tags")),
        PropValue::TextSpan(TextSpan::from("comments")),
        PropValue::TextSpan(TextSpan::from("date")),
    ]));

    let table = table(theme, items, profile)
        .custom("widths", widths)
        .custom("header", header);
    let browser: Browser<(PatchId, Patch)> = Browser::new(table);

    Widget::new(browser)
}

pub fn navigation(theme: &theme::Theme) -> Widget<Tabs> {
    tabs(
        theme,
        vec![label("dashboard"), label("issues"), label("patches")],
    )
}

pub fn dashboard(theme: &theme::Theme, id: &Id, project: &Project) -> Widget<LabeledContainer> {
    labeled_container(
        theme,
        "about",
        property_list(
            theme,
            vec![
                property(theme, "id", &id.to_string()),
                property(theme, "name", project.name()),
                property(theme, "description", project.description()),
            ],
        )
        .to_boxed(),
    )
}
