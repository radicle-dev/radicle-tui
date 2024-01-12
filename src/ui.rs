pub mod cob;
pub mod ext;
pub mod layout;
pub mod state;
pub mod subscription;
pub mod theme;
pub mod widget;

use tuirealm::props::{AttrValue, Attribute};
use tuirealm::MockComponent;

use widget::container::{
    AppHeader, AppInfo, Container, GlobalListener, Header, LabeledContainer, Popup, Tabs,
    VerticalLine,
};
use widget::context::{Shortcut, Shortcuts};
use widget::label::{self, Label, Textarea};
use widget::list::{ColumnWidth, Property, PropertyList, PropertyTable};
use widget::Widget;

use theme::{style, Theme};

use super::context::Context;

pub fn global_listener() -> Widget<GlobalListener> {
    Widget::new(GlobalListener::default())
}

pub fn container_header(theme: &Theme, label: Widget<Label>) -> Widget<Header<1>> {
    let header = Header::new([label], [ColumnWidth::Grow], theme.clone());

    Widget::new(header)
}

pub fn container(theme: &Theme, component: Box<dyn MockComponent>) -> Widget<Container> {
    let container = Container::new(component, theme.clone());
    Widget::new(container)
}

pub fn labeled_container(
    theme: &Theme,
    title: &str,
    component: Box<dyn MockComponent>,
) -> Widget<LabeledContainer> {
    let header = container_header(
        theme,
        label::default(&format!(" {title} ")).style(style::reset()),
    );
    let container = LabeledContainer::new(header, component, theme.clone());

    Widget::new(container)
}

pub fn shortcut(theme: &Theme, short: &str, long: &str) -> Widget<Shortcut> {
    let short = label::default(short).style(style::gray());
    let long = label::default(long).style(style::gray_dim());
    let divider = label::default(&theme.icons.whitespace.to_string());

    // TODO: Remove when size constraints are implemented
    let short_w = short.query(Attribute::Width).unwrap().unwrap_size();
    let divider_w = divider.query(Attribute::Width).unwrap().unwrap_size();
    let long_w = long.query(Attribute::Width).unwrap().unwrap_size();
    let width = short_w.saturating_add(divider_w).saturating_add(long_w);

    let shortcut = Shortcut::new(short, divider, long);

    Widget::new(shortcut).height(1).width(width)
}

pub fn shortcuts(theme: &Theme, shortcuts: Vec<Widget<Shortcut>>) -> Widget<Shortcuts> {
    let divider =
        label::default(&format!(" {} ", theme.icons.shortcutbar_divider)).style(style::gray_dim());
    let shortcut_bar = Shortcuts::new(shortcuts, divider);

    Widget::new(shortcut_bar).height(1)
}

pub fn property(theme: &Theme, name: &str, value: &str) -> Widget<Property> {
    let name = label::default(name).style(style::cyan());
    let divider = label::default(&format!(" {} ", theme.icons.property_divider));
    let value = label::default(value).style(style::reset());

    // TODO: Remove when size constraints are implemented
    let name_w = name.query(Attribute::Width).unwrap().unwrap_size();
    let divider_w = divider.query(Attribute::Width).unwrap().unwrap_size();
    let value_w = value.query(Attribute::Width).unwrap().unwrap_size();
    let width = name_w.saturating_add(divider_w).saturating_add(value_w);

    let property = Property::new(name, value).with_divider(divider);

    Widget::new(property).height(1).width(width)
}

pub fn property_list(_theme: &Theme, properties: Vec<Widget<Property>>) -> Widget<PropertyList> {
    let property_list = PropertyList::new(properties);

    Widget::new(property_list)
}

pub fn property_table(_theme: &Theme, properties: Vec<Widget<Property>>) -> Widget<PropertyTable> {
    let table = PropertyTable::new(properties);

    Widget::new(table)
}

pub fn tabs(_theme: &Theme, tabs: Vec<Widget<Label>>) -> Widget<Tabs> {
    let tabs = Tabs::new(tabs);

    Widget::new(tabs).height(2)
}

pub fn app_info(context: &Context) -> Widget<AppInfo> {
    let project = label::default(context.project().name()).style(style::cyan());
    let rid = label::default(&format!(" ({})", context.id())).style(style::yellow());

    let project_w = project
        .query(Attribute::Width)
        .unwrap_or(AttrValue::Size(0))
        .unwrap_size();
    let rid_w = rid
        .query(Attribute::Width)
        .unwrap_or(AttrValue::Size(0))
        .unwrap_size();

    let info = AppInfo::new(project, rid);
    Widget::new(info).width(project_w.saturating_add(rid_w))
}

pub fn app_header(
    context: &Context,
    theme: &Theme,
    nav: Option<Widget<Tabs>>,
) -> Widget<AppHeader> {
    let line = label::default(&theme.icons.tab_overline.to_string()).style(style::magenta());
    let line = Widget::new(VerticalLine::new(line));
    let info = app_info(context);
    let header = AppHeader::new(nav, info, line);

    Widget::new(header)
}

pub fn info(theme: &Theme, message: &str) -> Widget<Popup> {
    let textarea = Widget::new(Textarea::default()).content(AttrValue::String(message.to_owned()));
    let container = labeled_container(theme, "Info", textarea.to_boxed());

    Widget::new(Popup::new(theme.clone(), container))
        .width(50)
        .height(20)
}

pub fn warning(theme: &Theme, message: &str) -> Widget<Popup> {
    let textarea = Widget::new(Textarea::default()).content(AttrValue::String(message.to_owned()));
    let container = labeled_container(theme, "Warning", textarea.to_boxed());

    Widget::new(Popup::new(theme.clone(), container))
        .width(50)
        .height(20)
}

pub fn error(theme: &Theme, message: &str) -> Widget<Popup> {
    let textarea = Widget::new(Textarea::default()).content(AttrValue::String(message.to_owned()));
    let container = labeled_container(theme, "Error", textarea.to_boxed());

    Widget::new(Popup::new(theme.clone(), container))
        .width(50)
        .height(20)
}
