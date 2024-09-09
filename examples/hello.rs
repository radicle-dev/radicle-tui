use anyhow::Result;

use ratatui::Viewport;
use termion::event::Key;

use ratatui::style::Color;
use ratatui::text::Text;

use radicle_tui as tui;

use tui::store;
use tui::ui::rm::widget::input::{TextArea, TextAreaProps};
use tui::ui::rm::widget::ToWidget;
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

#[tokio::main]
pub async fn main() -> Result<()> {
    let channel = Channel::default();
    let sender = channel.tx.clone();
    let app = App {
        alien: ALIEN.to_string(),
    };

    let scene = TextArea::default()
        .to_widget(sender.clone())
        .on_event(|key, _, _| match key {
            Key::Char('q') => Some(Message::Quit),
            _ => None,
        })
        .on_update(|app: &App| {
            TextAreaProps::default()
                .content(Text::styled(app.alien.clone(), Color::Rgb(85, 85, 255)))
                .handle_keys(false)
                .to_boxed_any()
                .into()
        });

    tui::rm(app, scene, Viewport::default(), channel).await?;

    Ok(())
}
