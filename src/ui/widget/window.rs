use std::hash::Hash;
use std::{collections::HashMap, marker::PhantomData};

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
    /// Internal properties
    props: WindowProps<Id>,
    /// All pages known
    pages: HashMap<Id, Widget<S, M>>,
}

impl<S, M, Id> Default for Window<S, M, Id> {
    fn default() -> Self {
        Self {
            props: WindowProps::default(),
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

    fn handle_event(&mut self, key: termion::event::Key) -> Option<Self::Message> {
        let page = self
            .props
            .current_page
            .as_ref()
            .and_then(|id| self.pages.get_mut(id));

        if let Some(page) = page {
            page.handle_event(key);
        }

        None
    }

    fn update(&mut self, state: &Self::State, props: Option<ViewProps>) {
        if let Some(props) = props.and_then(|props| props.inner::<WindowProps<Id>>()) {
            self.props = props;
        }

        let page = self
            .props
            .current_page
            .as_ref()
            .and_then(|id| self.pages.get_mut(id));

        if let Some(page) = page {
            page.update(state);
        }
    }

    fn render(&self, frame: &mut ratatui::Frame, _props: RenderProps) {
        let area = frame.size();

        let page = self
            .props
            .current_page
            .as_ref()
            .and_then(|id| self.pages.get(id));

        if let Some(page) = page {
            page.render(frame, RenderProps::from(area).focus(true));
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
    /// Internal props
    props: ShortcutsProps,
    /// Phantom
    phantom: PhantomData<(S, M)>,
}

impl<S, M> Shortcuts<S, M> {
    pub fn divider(mut self, divider: char) -> Self {
        self.props.divider = divider;
        self
    }

    pub fn shortcuts(mut self, shortcuts: &[(&str, &str)]) -> Self {
        self.props.shortcuts.clear();
        for (short, long) in shortcuts {
            self.props
                .shortcuts
                .push((short.to_string(), long.to_string()));
        }
        self
    }
}

impl<S, M> Default for Shortcuts<S, M> {
    fn default() -> Self {
        Self {
            props: ShortcutsProps::default(),
            phantom: PhantomData,
        }
    }
}

impl<S, M> View for Shortcuts<S, M> {
    type Message = M;
    type State = S;

    fn handle_event(&mut self, _key: Key) -> Option<Self::Message> {
        None
    }

    fn update(&mut self, _state: &Self::State, props: Option<ViewProps>) {
        if let Some(props) = props.and_then(|props| props.inner::<ShortcutsProps>()) {
            self.props = props;
        }
    }

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        use ratatui::widgets::Table;

        let mut shortcuts = self.props.shortcuts.iter().peekable();
        let mut row = vec![];

        while let Some(shortcut) = shortcuts.next() {
            let short = Text::from(shortcut.0.clone()).style(style::gray());
            let long = Text::from(shortcut.1.clone()).style(style::gray().dim());
            let spacer = Text::from(String::new());
            let divider =
                Text::from(format!(" {} ", self.props.divider)).style(style::gray().dim());

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
        frame.render_widget(table, props.area);
    }
}
