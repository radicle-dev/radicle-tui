use termion::event::Key;

use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

use radicle_tui as tui;

use tui::ui::im::widget::{TableState, TextEditState, Widget};
use tui::ui::im::{Borders, BufferedValue, Response, Ui};
use tui::ui::rm::widget::container::Column;
use tui::ui::rm::widget::list::ToRow;

pub struct UiExt<'a>(&'a mut Ui);

impl<'a> UiExt<'a> {
    pub fn new(ui: &'a mut Ui) -> Self {
        Self(ui)
    }
}

impl<'a> From<&'a mut Ui> for UiExt<'a> {
    fn from(ui: &'a mut Ui) -> Self {
        Self::new(ui)
    }
}

#[allow(dead_code)]
impl<'a> UiExt<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn browser<R, const W: usize>(
        &mut self,
        frame: &mut Frame,
        selected: &'a mut Option<usize>,
        items: &'a Vec<R>,
        header: impl IntoIterator<Item = Column<'a>>,
        footer: impl IntoIterator<Item = Column<'a>>,
        show_search: &'a mut bool,
        search: &'a mut BufferedValue<TextEditState>,
    ) -> Response
    where
        R: ToRow<W> + Clone,
    {
        Browser::<R, W>::new(selected, items, header, footer, show_search, search).ui(self.0, frame)
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct BrowserState {
    items: TableState,
    search: BufferedValue<TextEditState>,
    show_search: bool,
}

#[allow(dead_code)]
impl BrowserState {
    pub fn new(items: TableState, search: BufferedValue<TextEditState>, show_search: bool) -> Self {
        Self {
            items,
            search,
            show_search,
        }
    }

    pub fn selected(&self) -> Option<usize> {
        self.items.selected()
    }
}

pub struct Browser<'a, R, const W: usize> {
    items: &'a Vec<R>,
    selected: &'a mut Option<usize>,
    header: Vec<Column<'a>>,
    footer: Vec<Column<'a>>,
    show_search: &'a mut bool,
    search: &'a mut BufferedValue<TextEditState>,
}

#[allow(dead_code)]
impl<'a, R, const W: usize> Browser<'a, R, W> {
    pub fn new(
        selected: &'a mut Option<usize>,
        items: &'a Vec<R>,
        header: impl IntoIterator<Item = Column<'a>>,
        footer: impl IntoIterator<Item = Column<'a>>,
        show_search: &'a mut bool,
        search: &'a mut BufferedValue<TextEditState>,
    ) -> Self {
        Self {
            items,
            selected,
            header: header.into_iter().collect(),
            footer: footer.into_iter().collect(),
            show_search,
            search,
        }
    }

    pub fn items(&self) -> &Vec<R> {
        self.items
    }
}

/// TODO(erikli): Implement `show` that returns an `InnerResponse` such that it can
/// used like a group.
impl<'a, R, const W: usize> Widget for Browser<'a, R, W>
where
    R: ToRow<W> + Clone,
{
    fn ui(self, ui: &mut Ui, frame: &mut Frame) -> Response {
        let mut response = Response::default();

        let (_, has_focus) = ui.current_area().unwrap_or_default();
        let (mut text, mut cursor) = (self.search.read().text, self.search.read().cursor);

        ui.layout(
            Layout::vertical([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(if *self.show_search { 2 } else { 3 }),
            ]),
            |ui| {
                // TODO(erikli): Find better solution for border focus workaround or improve
                // interface for manually advancing / setting the focus index.
                if has_focus && !*self.show_search {
                    ui.set_focus(Some(0));
                }
                ui.columns(frame, self.header.clone().to_vec(), Some(Borders::Top));

                if has_focus && !*self.show_search {
                    ui.set_focus(Some(1));
                }
                let table = ui.table(
                    frame,
                    self.selected,
                    self.items,
                    self.header.to_vec(),
                    if *self.show_search {
                        Some(Borders::BottomSides)
                    } else {
                        Some(Borders::Sides)
                    },
                );
                response.changed |= table.changed;

                if *self.show_search {
                    if has_focus {
                        ui.set_focus(Some(2));
                    }
                    let text_edit = ui.text_edit_labeled_singleline(
                        frame,
                        &mut text,
                        &mut cursor,
                        "Search".to_string(),
                        Some(Borders::Spacer { top: 0, left: 1 }),
                    );
                    self.search.write(TextEditState { text, cursor });
                    response.changed |= text_edit.changed;
                } else {
                    if has_focus {
                        ui.set_focus(Some(2));
                    }
                    ui.columns(frame, self.footer.clone().to_vec(), Some(Borders::Bottom));
                }
            },
        );

        if !*self.show_search {
            if ui.input_global(|key| key == Key::Char('/')) {
                *self.show_search = true;
            }
        } else {
            if ui.input_global(|key| key == Key::Esc) {
                *self.show_search = false;
                self.search.reset();
            }
            if ui.input_global(|key| key == Key::Char('\n')) {
                *self.show_search = false;
                self.search.apply();
            }
        }

        response
    }
}
