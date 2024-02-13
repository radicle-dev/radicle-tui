use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::backend::Backend;
use ratatui::widgets::Paragraph;

use radicle_tui as tui;
use tui::flux::ui::{Render, Widget};

use crate::tui_inbox::select::flux::{Action, InboxState};

pub struct Props {
    notifications: Vec<String>,
}

impl From<&InboxState> for Props {
    fn from(state: &InboxState) -> Self {
        Props {
            notifications: state.notifications().clone(),
        }
    }
}

pub struct ListPage {
    /// Action sender
    pub action_tx: UnboundedSender<Action>,
    // Mapped Props from State
    props: Props,
}

impl Widget<InboxState, Action> for ListPage {
    fn new(state: &InboxState, action_tx: UnboundedSender<Action>) -> Self
    where
        Self: Sized,
    {
        ListPage {
            action_tx: action_tx.clone(),
            props: Props::from(state),
        }
        .move_with_state(state)
    }

    fn move_with_state(self, state: &InboxState) -> Self
    where
        Self: Sized,
    {
        ListPage {
            props: Props::from(state),
            ..self
        }
    }

    fn name(&self) -> &str {
        "list-page"
    }

    fn handle_key_event(&mut self, key: termion::event::Key) {
        match key {
            Key::Char('q') => {
                let _ = self.action_tx.send(Action::Exit);
            }
            _ => {}
        }
    }
}

impl Render<()> for ListPage {
    fn render<B: Backend>(&self, frame: &mut ratatui::Frame, _props: ()) {
        let area = frame.size();
        let layout = tui::flux::ui::layout::default_page(area, 1u16, 1u16);

        let shortcuts = Paragraph::new(String::from("q quit"));

        frame.render_widget(shortcuts, layout.shortcuts);
    }
}
