use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use termion::event::Key;

use ratatui::layout::{Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::text::Span;
use ratatui::widgets::Cell;
use ratatui::{Frame, Viewport};

use radicle_tui as tui;

use tui::ui::im::widget::Window;
use tui::ui::im::{Borders, Context};
use tui::ui::{Column, ToRow};
use tui::Channel;
use tui::{
    store::Update,
    ui::im::{widget::TableState, Show},
    Exit,
};

#[derive(Clone, Debug)]
struct Item {
    id: usize,
    title: String,
    timestamp: usize,
}

impl ToRow<3> for Item {
    fn to_row(&self) -> [Cell; 3] {
        [
            Span::raw(self.id.to_string()).magenta().dim().into(),
            Span::raw(self.title.clone()).into(),
            Span::raw(self.timestamp.to_string())
                .dark_gray()
                .italic()
                .into(),
        ]
    }
}

#[derive(Clone, Debug)]
struct App {
    items: Vec<Item>,
    selector: TableState,
}

#[derive(Clone, Debug)]
enum Message {
    SelectionChanged { state: TableState },
    Return,
    Quit,
}

impl Update<Message> for App {
    type Return = usize;

    fn update(&mut self, message: Message) -> Option<tui::Exit<Self::Return>> {
        match message {
            Message::SelectionChanged { state } => {
                self.selector = state;
                None
            }
            Message::Return => self
                .selector
                .selected()
                .and_then(|selected| self.items.get(selected))
                .map(|item| Exit {
                    value: Some(item.id),
                }),
            Message::Quit => Some(Exit { value: None }),
        }
    }
}

impl Show<Message> for App {
    fn show(&self, ctx: &Context<Message>, frame: &mut Frame) -> Result<()> {
        Window::default().show(ctx, |ui| {
            ui.layout(
                Layout::vertical([
                    Constraint::Length(1),
                    Constraint::Min(1),
                    Constraint::Length(1),
                ]),
                Some(1),
                |ui| {
                    let columns = [
                        Column::new(Span::raw("Id").bold(), Constraint::Length(4)),
                        Column::new(Span::raw("Title").bold(), Constraint::Fill(1)),
                        Column::new(Span::raw("Timestamp").bold(), Constraint::Fill(1)),
                    ]
                    .to_vec();
                    let mut selected = self.selector.selected();

                    ui.columns(frame, columns.clone(), Some(Borders::None));

                    let table = ui.table(
                        frame,
                        &mut selected,
                        &self.items,
                        columns,
                        Some(Borders::None),
                    );
                    if table.changed {
                        ui.send_message(Message::SelectionChanged {
                            state: TableState::new(selected),
                        })
                    }

                    ui.shortcuts(frame, &[("q", "quit")], '|');

                    if ui.input_global(|key| key == Key::Char('\n')) {
                        ui.send_message(Message::Return);
                    }
                },
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
        items: (0..200)
            .map(|id| Item {
                id,
                title: format!("Title of item #{}", id),
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("Could not read system time")
                    .as_secs() as usize,
            })
            .collect(),
        selector: TableState::new(Some(0)),
    };

    if let Some(exit) = tui::im(app, Viewport::Inline(12), Channel::default()).await? {
        println!("{exit}");
    } else {
        anyhow::bail!("No selection");
    }

    Ok(())
}
