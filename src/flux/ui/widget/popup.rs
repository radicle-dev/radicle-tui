use std::cmp;
use std::fmt::Debug;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, Cell, Row, TableState};

use super::{layout, span};
use super::{Render, Widget};

pub struct FilterPopupProps {}

pub struct FilterPopup<A> {
    pub action_tx: UnboundedSender<A>,
}

impl<S, A> Widget<S, A> for FilterPopup<A> {
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
        "filter-popup"
    }

    fn handle_key_event(&mut self, _key: termion::event::Key) {}
}

impl<A> Render<FilterPopupProps> for FilterPopup<A> {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, area: Rect, props: FilterPopupProps) {
        
    }
}
