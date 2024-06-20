use anyhow::Result;

use termion::event::Key;

use ratatui::style::Color;
use ratatui::text::Text;

use radicle_tui as tui;

use tui::store;
use tui::ui::widget::input::{TextArea, TextAreaProps};
use tui::ui::widget::ToWidget;
use tui::{BoxedAny, Channel, Exit};

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

#[tokio::main]
pub async fn main() -> Result<()> {
    let channel = Channel::default();
    let sender = channel.tx.clone();
    let state = State {
        alien: ALIEN.to_string(),
    };

    let scene = TextArea::default()
        .to_widget(sender.clone())
        .on_event(|key, _, _| match key {
            Key::Char('q') => Some(Message::Quit),
            _ => None,
        })
        .on_update(|state: &State| {
            TextAreaProps::default()
                .content(Text::styled(state.alien.clone(), Color::Rgb(85, 85, 255)))
                .handle_keys(false)
                .to_boxed_any()
                .into()
        });

    tui::run(channel, state, scene).await?;

    Ok(())
}
