use anyhow::Result;

use termion::event::Key;

use ratatui::layout::Constraint;

use radicle_tui as tui;

use tui::store;
use tui::ui::widget::container::{Column, Container, Header, HeaderProps};
use tui::ui::widget::input::{TextArea, TextAreaProps};
use tui::ui::widget::window::{Page, Shortcuts, ShortcutsProps, Window, WindowProps};
use tui::ui::widget::ToWidget;
use tui::{BoxedAny, Channel, Exit};

const CONTENT: &str = r#"
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor
incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud 
exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure 
dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.

Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt 
mollit anim id est laborum.
"#;

#[derive(Clone, Debug)]
struct State {
    content: String,
}

enum Message {
    Quit,
    ReverseContent,
}

impl store::State<()> for State {
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> Option<tui::Exit<()>> {
        match message {
            Message::Quit => Some(Exit { value: None }),
            Message::ReverseContent => {
                self.content = self.content.chars().rev().collect::<String>();
                None
            }
        }
    }
}

#[tokio::main]
pub async fn main() -> Result<()> {
    let channel = Channel::default();
    let sender = channel.tx.clone();
    let state = State {
        content: CONTENT.to_string(),
    };

    let page = Page::default()
        .content(
            Container::default()
                .header(Header::default().to_widget(sender.clone()).on_update(|_| {
                    HeaderProps::default()
                        .columns(vec![
                            Column::new("", Constraint::Length(0)),
                            Column::new(
                                "The standard Lorem Ipsum passage, used since the 1500s",
                                Constraint::Fill(1),
                            ),
                        ])
                        .to_boxed_any()
                        .into()
                }))
                .content(TextArea::default().to_widget(sender.clone()).on_update(
                    |state: &State| {
                        TextAreaProps::default()
                            .content(state.content.clone())
                            .can_scroll(false)
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
        .page(0, page)
        .to_widget(sender.clone())
        .on_event(|key, _, _| match key {
            Key::Char('r') => Some(Message::ReverseContent),
            Key::Char('q') => Some(Message::Quit),
            _ => None,
        })
        .on_update(|_| WindowProps::default().current_page(0).to_boxed_any().into());

    tui::run(channel, state, window).await?;

    Ok(())
}
