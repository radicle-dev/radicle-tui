use std::hash::Hash;
use std::{collections::HashMap, marker::PhantomData};

use ratatui::Frame;
use termion::event::Key;

use ratatui::layout::Constraint;
use ratatui::style::Stylize;
use ratatui::text::Text;
use ratatui::widgets::Row;

use crate::ui::theme::style;

use super::{RenderProps, View, ViewProps, Widget};

#[derive(Clone)]
pub struct WindowProps<Id> {
    current_page: Option<Id>,
}

impl<Id> WindowProps<Id> {
    pub fn current_page(mut self, page: Id) -> Self {
        self.current_page = Some(page);
        self
    }
}

impl<Id> Default for WindowProps<Id> {
    fn default() -> Self {
        Self { current_page: None }
    }
}

pub struct Window<S, M, Id> {
    /// All pages known
    pages: HashMap<Id, Widget<S, M>>,
}

impl<S, M, Id> Default for Window<S, M, Id> {
    fn default() -> Self {
        Self {
            pages: HashMap::new(),
        }
    }
}

impl<S, M, Id> Window<S, M, Id>
where
    Id: Clone + Hash + Eq + PartialEq,
{
    pub fn page(mut self, id: Id, page: Widget<S, M>) -> Self {
        self.pages.insert(id, page);
        self
    }
}

impl<'a, S, M, Id> View for Window<S, M, Id>
where
    'a: 'static,
    S: 'static,
    M: 'static,
    Id: Clone + Hash + Eq + PartialEq + 'static,
{
    type Message = M;
    type State = S;

    fn handle_event(&mut self, props: Option<&ViewProps>, key: Key) -> Option<Self::Message> {
        let default = WindowProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<WindowProps<Id>>())
            .unwrap_or(&default);

        let page = props
            .current_page
            .as_ref()
            .and_then(|id| self.pages.get_mut(id));

        if let Some(page) = page {
            page.handle_event(key);
        }

        None
    }

    fn update(&mut self, props: Option<&ViewProps>, state: &Self::State) {
        let default = WindowProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<WindowProps<Id>>())
            .unwrap_or(&default);

        let page = props
            .current_page
            .as_ref()
            .and_then(|id| self.pages.get_mut(id));

        if let Some(page) = page {
            page.update(state);
        }
    }

    fn render(&self, props: Option<&ViewProps>, _render: RenderProps, frame: &mut Frame) {
        let default = WindowProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<WindowProps<Id>>())
            .unwrap_or(&default);

        let area = frame.size();

        let page = props
            .current_page
            .as_ref()
            .and_then(|id| self.pages.get(id));

        if let Some(page) = page {
            page.render(RenderProps::from(area).focus(true), frame);
        }
    }
}

#[derive(Clone)]
pub struct ShortcutsProps {
    pub shortcuts: Vec<(String, String)>,
    pub divider: char,
}

impl ShortcutsProps {
    pub fn divider(mut self, divider: char) -> Self {
        self.divider = divider;
        self
    }

    pub fn shortcuts(mut self, shortcuts: &[(&str, &str)]) -> Self {
        self.shortcuts.clear();
        for (short, long) in shortcuts {
            self.shortcuts.push((short.to_string(), long.to_string()));
        }
        self
    }
}

impl Default for ShortcutsProps {
    fn default() -> Self {
        Self {
            shortcuts: vec![],
            divider: '∙',
        }
    }
}

pub struct Shortcuts<S, M> {
    /// Phantom
    phantom: PhantomData<(S, M)>,
}

impl<S, M> Default for Shortcuts<S, M> {
    fn default() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<S, M> View for Shortcuts<S, M> {
    type Message = M;
    type State = S;

    fn render(&self, props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame) {
        use ratatui::widgets::Table;

        let default = ShortcutsProps::default();
        let props = props
            .and_then(|props| props.inner_ref::<ShortcutsProps>())
            .unwrap_or(&default);

        let mut shortcuts = props.shortcuts.iter().peekable();
        let mut row = vec![];

        while let Some(shortcut) = shortcuts.next() {
            let short = Text::from(shortcut.0.clone()).style(style::gray());
            let long = Text::from(shortcut.1.clone()).style(style::gray().dim());
            let spacer = Text::from(String::new());
            let divider = Text::from(format!(" {} ", props.divider)).style(style::gray().dim());

            row.push((shortcut.0.chars().count(), short));
            row.push((1, spacer));
            row.push((shortcut.1.chars().count(), long));

            if shortcuts.peek().is_some() {
                row.push((3, divider));
            }
        }

        let row_copy = row.clone();
        let row: Vec<Text<'_>> = row_copy
            .clone()
            .iter()
            .map(|(_, text)| text.clone())
            .collect();
        let widths: Vec<Constraint> = row_copy
            .clone()
            .iter()
            .map(|(width, _)| Constraint::Length(*width as u16))
            .collect();

        let table = Table::new([Row::new(row)], widths).column_spacing(0);
        frame.render_widget(table, render.area);
    }
}
