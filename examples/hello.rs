use anyhow::Result;

use radicle_tui as tui;

use termion::event::Key;
use tui::store;
use tui::ui::widget::text::{Paragraph, ParagraphProps};
use tui::ui::widget::{Properties, Widget};
use tui::{Channel, Exit};

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

    let welcome = Paragraph::new(&state, channel.tx.clone())
        .on_update(|state| {
            ParagraphProps::default()
                .text(&state.welcome.clone().into())
                .to_boxed()
        })
        .on_event(|paragraph, key| {
            paragraph
                .downcast_mut::<Paragraph<'_, State, Message>>()
                .and_then(|paragraph| match key {
                    Key::Char('r') => paragraph.send(Message::ReverseWelcome).ok(),
                    Key::Char('q') => paragraph.send(Message::Quit).ok(),
                    _ => None,
                });
        })
        .to_boxed();

    tui::run(channel, state, welcome).await?;

    Ok(())
}
