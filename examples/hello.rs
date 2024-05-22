use anyhow::Result;

use radicle_tui as tui;

use termion::event::Key;
use tui::store;
use tui::ui::widget::text::{Paragraph, ParagraphProps};
use tui::ui::widget::ToWidget;
use tui::{BoxedAny, Channel, Exit};

#[derive(Clone, Debug)]
struct State {
    welcome: String,
}

enum Message {
    Quit,
    ReverseWelcome,
}

impl store::State<()> for State {
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> Option<tui::Exit<()>> {
        match message {
            Message::Quit => Some(Exit { value: None }),
            Message::ReverseWelcome => {
                self.welcome = self.welcome.chars().rev().collect::<String>();
                None
            }
        }
    }

    fn tick(&self) {}
}

#[tokio::main]
pub async fn main() -> Result<()> {
    let channel = Channel::default();
    let state = State {
        welcome: "Hello TUI".to_string(),
    };

    let welcome = Paragraph::default()
        .to_widget(channel.tx.clone())
        .on_update(|state: &State| {
            ParagraphProps::default()
                .text(&state.welcome.clone().into())
                .to_boxed_any()
                .into()
        })
        .on_event(|_, key| match key {
            Key::Char('r') => Some(Message::ReverseWelcome),
            Key::Char('q') => Some(Message::Quit),
            _ => None,
        });

    tui::run(channel, state, welcome).await?;

    Ok(())
}
