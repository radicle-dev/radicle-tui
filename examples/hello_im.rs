use anyhow::Result;

use termion::event::Key;

use ratatui::Frame;

use radicle_tui as tui;

use tui::store;
use tui::ui::im;
use tui::ui::im::widget::Window;
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
struct State {
    alien: String,
}

#[derive(Clone)]
enum Message {
    Quit,
}

impl store::State<()> for State {
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> Option<tui::Exit<()>> {
        match message {
            Message::Quit => Some(Exit { value: None }),
        }
    }
}

#[derive(Default)]
struct App {}

impl im::App for App {
    type State = State;
    type Message = Message;

    fn update(&self, ctx: &Context<Message>, frame: &mut Frame, state: &State) -> Result<()> {
        Window::default().show(ctx, |ui| {
            ui.text_view(frame, state.alien.clone(), &mut (0, 0), Some(Borders::None));

            if ui.input_global(|key| key == Key::Char('q')) {
                ui.send_message(Message::Quit);
            }
        });

        Ok(())
    }
}

#[tokio::main]
pub async fn main() -> Result<()> {
    let state = State {
        alien: ALIEN.to_string(),
    };
    tui::im(Channel::default(), state, App::default()).await?;

    Ok(())
}
