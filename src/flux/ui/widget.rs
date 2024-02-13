use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::prelude::*;
use ratatui::widgets::{Row, Table};

use super::theme::style;

pub trait Widget<S, A> {
    fn new(state: &S, action_tx: UnboundedSender<A>) -> Self
    where
        Self: Sized;

    fn move_with_state(self, state: &S) -> Self
    where
        Self: Sized;

    fn name(&self) -> &str;

    fn handle_key_event(&mut self, key: Key);
}

pub trait Render<P> {
    fn render<B: ratatui::backend::Backend>(&self, frame: &mut Frame, area: Rect, props: P);
}

///
///
///
pub struct Shortcut {
    pub short: String,
    pub long: String,
}

impl Shortcut {
    pub fn new(short: &str, long: &str) -> Self {
        Self {
            short: short.to_string(),
            long: long.to_string(),
        }
    }
}

pub struct ShortcutWidgetProps {
    pub shortcuts: Vec<Shortcut>,
    pub divider: char,
}

pub struct ShortcutWidget<A> {
    pub action_tx: UnboundedSender<A>,
}

impl<S, A> Widget<S, A> for ShortcutWidget<A> {
    fn new(state: &S, action_tx: UnboundedSender<A>) -> Self
    where
        Self: Sized,
    {
        Self {
            action_tx: action_tx.clone(),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, _state: &S) -> Self
    where
        Self: Sized,
    {
        Self { ..self }
    }

    fn name(&self) -> &str {
        "shortcuts"
    }

    fn handle_key_event(&mut self, _key: termion::event::Key) {}
}

impl<A> Render<ShortcutWidgetProps> for ShortcutWidget<A> {
    fn render<B: Backend>(
        &self,
        frame: &mut ratatui::Frame,
        area: Rect,
        props: ShortcutWidgetProps,
    ) {
        let mut shortcuts = props.shortcuts.iter().peekable();
        let mut row = vec![];

        while let Some(shortcut) = shortcuts.next() {
            let short = Text::from(shortcut.short.clone()).style(style::gray());
            let long = Text::from(shortcut.long.clone()).style(style::gray_dim());
            let spacer = Text::from(String::new());
            let divider =
                Text::from(String::from(format!(" {} ", props.divider))).style(style::gray_dim());

            row.push((1, short));
            row.push((1, spacer));
            row.push((shortcut.long.len(), long));

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
        frame.render_widget(table, area);
    }
}
