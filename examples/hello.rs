use anyhow::Result;

use ratatui::layout::Position;
use termion::event::Key;

use ratatui::{Frame, Viewport};

use radicle_tui as tui;

use tui::store;
use tui::task::EmptyProcessors;
use tui::ui::im::widget::Window;
use tui::ui::im::Show;
use tui::ui::im::{Borders, Context};
use tui::{Channel, Exit};

const ALIEN: &str = r#"
     ///             ///    ,---------------------------------.
     ///             ///    | Hey there, press (q) to quit... |
        //         //       '---------------------------------'
        //,,,///,,,//      ..
     ///////////////////  .
  //////@@@@@//////@@@@@///
  //////@@###//////@@###///
,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,
     ,,,  ///   ///  ,,,
     ,,,  ///   ///  ,,,
          ///   ///
        /////   /////
"#;

#[derive(Clone, Debug)]
struct App {
    alien: String,
}

#[derive(Clone, Debug)]
enum Message {
    Quit,
}

impl store::Update<Message> for App {
    type Return = ();

    fn update(&mut self, message: Message) -> Option<tui::Exit<()>> {
        match message {
            Message::Quit => Some(Exit { value: None }),
        }
    }
}

impl Show<Message> for App {
    fn show(&self, ctx: &Context<Message>, frame: &mut Frame) -> Result<()> {
        Window::default().show(ctx, |ui| {
            ui.text_view(
                frame,
                self.alien.clone(),
                &mut Position::default(),
                Some(Borders::None),
            );

            if ui.input_global(|key| key == Key::Char('q')) {
                ui.send_message(Message::Quit);
            }
        });

        Ok(())
    }
}

#[tokio::main]
pub async fn main() -> Result<()> {
    let app = App {
        alien: ALIEN.to_string(),
    };

    tui::im(
        app,
        Viewport::default(),
        Channel::default(),
        EmptyProcessors::new(),
    )
    .await?;

    Ok(())
}
