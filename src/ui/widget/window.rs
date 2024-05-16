use std::collections::HashMap;
use std::hash::Hash;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::layout::Constraint;
use ratatui::style::Stylize;
use ratatui::text::Text;
use ratatui::widgets::Row;

use crate::ui::theme::style;

use super::{BoxedAny, BoxedWidget, Properties, RenderProps, Widget, WidgetBase};

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

impl<Id> Properties for WindowProps<Id> {}
impl<Id> BoxedAny for WindowProps<Id> {}

pub struct Window<S, M, Id> {
    /// Internal base
    base: WidgetBase<S, M>,
    /// Internal properties
    props: WindowProps<Id>,
    /// All pages known
    pages: HashMap<Id, BoxedWidget<S, M>>,
}

impl<S, M, Id> Window<S, M, Id>
where
    Id: Clone + Hash + Eq + PartialEq,
{
    pub fn page(mut self, id: Id, page: BoxedWidget<S, M>) -> Self {
        self.pages.insert(id, page);
        self
    }
}

impl<'a, S, M, Id> Widget for Window<S, M, Id>
where
    'a: 'static,
    Id: Clone + Hash + Eq + PartialEq + 'static,
{
    type Message = M;
    type State = S;

    fn new(_state: &S, tx: UnboundedSender<M>) -> Self
    where
        Self: Sized,
    {
        Self {
            base: WidgetBase::new(tx.clone()),
            props: WindowProps::default(),
            pages: HashMap::new(),
        }
    }

    fn handle_event(&mut self, key: termion::event::Key) {
        let page = self
            .props
            .current_page
            .as_ref()
            .and_then(|id| self.pages.get_mut(id));

        if let Some(page) = page {
            page.handle_event(key);
        }
    }

    fn update(&mut self, state: &S) {
        self.props =
            WindowProps::from_callback(self.base.on_update, state).unwrap_or(self.props.clone());

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

    fn base(&self) -> &WidgetBase<S, M> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut WidgetBase<S, M> {
        &mut self.base
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

impl Properties for ShortcutsProps {}
impl BoxedAny for ShortcutsProps {}

pub struct Shortcuts<S, M> {
    /// Internal properties
    props: ShortcutsProps,
    /// Internal base
    base: WidgetBase<S, M>,
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

impl<S, M> Widget for Shortcuts<S, M> {
    type Message = M;
    type State = S;

    fn new(_state: &S, tx: UnboundedSender<M>) -> Self {
        Self {
            base: WidgetBase::new(tx.clone()),
            props: ShortcutsProps::default(),
        }
    }

    fn handle_event(&mut self, _key: Key) {}

    fn update(&mut self, state: &S) {
        self.props =
            ShortcutsProps::from_callback(self.base.on_update, state).unwrap_or(self.props.clone());
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

    fn base(&self) -> &WidgetBase<S, M> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut WidgetBase<S, M> {
        &mut self.base
    }
}
