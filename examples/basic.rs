use anyhow::Result;

use ratatui::Viewport;
use termion::event::Key;

use ratatui::layout::Constraint;

use radicle_tui as tui;

use tui::store;
use tui::ui::rm::widget::container::{Container, Header, HeaderProps};
use tui::ui::rm::widget::input::{TextView, TextViewProps, TextViewState};
use tui::ui::rm::widget::window::{Page, Shortcuts, ShortcutsProps, Window, WindowProps};
use tui::ui::rm::widget::ToWidget;
use tui::ui::Column;
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
struct App {
    content: String,
}

#[derive(Clone, Debug)]
enum Message {
    Quit,
    ReverseContent,
}

impl store::Update<Message> for App {
    type Return = ();

    fn update(&mut self, message: Message) -> Option<tui::Exit<()>> {
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
    let app = App {
        content: CONTENT.to_string(),
    };

    let page =
        Page::default()
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
                    .content(TextView::default().to_widget(sender.clone()).on_update(
                        |app: &App| {
                            let content = app.content.clone();
                            TextViewProps::default()
                                .state(Some(TextViewState::default().content(content)))
                                .handle_keys(false)
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

    tui::rm(app, window, Viewport::default(), channel).await?;

    Ok(())
}
