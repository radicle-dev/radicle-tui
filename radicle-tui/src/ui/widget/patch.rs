use radicle_surf::diff::Diff;

use radicle::Profile;
use radicle_term as term;

use term::Line;
use term::Paint;
use tuirealm::props::Color;
use tuirealm::AttrValue;

use radicle::cob::patch::{Patch, PatchId};
use tuirealm::props::PropPayload;
use tuirealm::props::PropValue;
use tuirealm::props::TextModifiers;
use tuirealm::props::TextSpan;
use tuirealm::tui::widgets::Cell;
use tuirealm::tui::widgets::Row;

use super::common;
use super::Widget;

use crate::ui::cob::patch;
use crate::ui::components::common::container::Tabs;
use crate::ui::components::common::context::ContextBar;
use crate::ui::components::common::list::Table;
use crate::ui::components::patch::Activity;
use crate::ui::components::patch::Files;
use crate::ui::theme::Theme;

fn color_to_color(color: term::Color) -> Color {
    match color {
        term::Color::Green => Color::Green,
        term::Color::Red => Color::Red,
        term::Color::Fixed(236) => Color::DarkGray,
        _ => Color::Reset,
    }
}

fn line_to_span(line: &Line) -> TextSpan {
    // for label in line.items() {
    //     TextSpan::from(label)
    // }
    match line.items().get(0) {
        Some(label) => {
            let paint: Paint<String> = label.clone().into();
            let style = paint.style();
            TextSpan::from(paint.content())
                .fg(color_to_color(style.fg_color()))
            // if bold {
            //     span = span.add_modifiers(TextModifiers::BOLD);
            // }
        }
        None => TextSpan::from(""),
    }
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

pub fn activity(theme: &Theme, patch: (PatchId, &Patch), profile: &Profile) -> Widget<Activity> {
    let (id, patch) = patch;
    let shortcuts = common::shortcuts(
        theme,
        vec![
            common::shortcut(theme, "esc", "back"),
            common::shortcut(theme, "tab", "section"),
            common::shortcut(theme, "q", "quit"),
        ],
    );
    let context = context(theme, (id, patch), profile);

    let not_implemented = common::label("not implemented").foreground(theme.colors.default_fg);
    let activity = Activity::new(not_implemented, context, shortcuts);

    Widget::new(activity)
}

pub fn files(
    theme: &Theme,
    patch: (PatchId, &Patch),
    diff: &Diff,
    profile: &Profile,
) -> Widget<Files> {
    let (id, patch) = patch;

    let mut items = vec![];
    let mut files = diff.files();
    for file in files {
        if let Ok(header) = term::format::diff::file_header(file) {
            let cells = header
                .iter()
                .map(|line| line_to_span(line))
                .collect::<Vec<_>>();
            items.push(cells);

            if let Ok(rows) = term::format::diff::file_rows(file) {
                for row in rows {
                    let cells = row
                        .iter()
                        .map(|line| line_to_span(line))
                        .collect::<Vec<_>>();
                    items.push(cells);
                }
            }
        }
    }

    let labels = vec!["", "", ""];
    let widths = vec![3u16, 3, 94];

    let header = common::table_header(theme, &labels, &widths);
    let table = Table::new(header);

    let widths = AttrValue::Payload(PropPayload::Vec(
        widths.iter().map(|w| PropValue::U16(*w)).collect(),
    ));

    let table = Widget::new(table)
        .content(AttrValue::Table(items))
        .custom("widths", widths)
        .background(theme.colors.labeled_container_bg)
        .highlight(theme.colors.item_list_highlighted_bg);

    let context = context(theme, (id, patch), profile);
    let shortcuts = common::shortcuts(
        theme,
        vec![
            common::shortcut(theme, "esc", "back"),
            common::shortcut(theme, "tab", "section"),
            common::shortcut(theme, "q", "quit"),
        ],
    );

    let files = Files::new(table, context, shortcuts);

    Widget::new(files)
}

pub fn context(_theme: &Theme, patch: (PatchId, &Patch), profile: &Profile) -> Widget<ContextBar> {
    let (id, patch) = patch;
    let id = patch::format_id(id);
    let title = patch.title();
    let author = patch::format_author(patch, profile);
    let comments = patch::format_comments(patch);

    let context = common::label(" patch ").background(Color::Rgb(238, 111, 248));
    let id = common::label(&format!(" {id} "))
        .foreground(Color::Rgb(117, 113, 249))
        .background(Color::Rgb(40, 40, 40));
    let title = common::label(&format!(" {title} "))
        .foreground(Color::Rgb(70, 70, 70))
        .background(Color::Rgb(40, 40, 40));
    let author = common::label(&format!(" {author} "))
        .foreground(Color::Rgb(117, 113, 249))
        .background(Color::Rgb(40, 40, 40));
    let comments = common::label(&format!(" {comments} "))
        .foreground(Color::Rgb(70, 70, 70))
        .background(Color::Rgb(50, 50, 50));

    let context_bar = ContextBar::new(context, id, author, title, comments);

    Widget::new(context_bar).height(1)
}
