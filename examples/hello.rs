use anyhow::Result;

use radicle_tui as tui;

use termion::event::Key;
use tui::store;
use tui::ui::widget::container::Container;
use tui::ui::widget::text::{Paragraph, ParagraphProps};
use tui::ui::widget::window::{Page, Shortcuts, ShortcutsProps, Window, WindowProps};
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
    let sender = channel.tx.clone();
    let state = State {
        welcome: "Hello TUI".to_string(),
    };

    let welcome = Page::default()
        .content(
            Container::default()
                .content(Paragraph::default().to_widget(sender.clone()).on_update(
                    |state: &State| {
                        ParagraphProps::default()
                            .text(&state.welcome.clone().into())
                            .to_boxed_any()
                            .into()
                    },
                ))
                .to_widget(sender.clone()),
        )
        .shortcuts(
            Shortcuts::default()
                .to_widget(sender.clone())
                .on_update(|_| {
                    ShortcutsProps::default()
                        .shortcuts(&[("q", "quit"), ("r", "reverse")])
                        .to_boxed_any()
                        .into()
                }),
        )
        .to_widget(sender.clone());

    let window = Window::default()
        .page(0, welcome)
        .to_widget(sender.clone())
        .on_event(|key, _, _| match key {
            Key::Char('r') => Some(Message::ReverseWelcome),
            Key::Char('q') => Some(Message::Quit),
            _ => None,
        })
        .on_update(|_| WindowProps::default().current_page(0).to_boxed_any().into());

    tui::run(channel, state, window).await?;

    Ok(())
}
