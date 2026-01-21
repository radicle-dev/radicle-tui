use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::text::Span;
use ratatui::widgets::Cell;
use ratatui::{Frame, Viewport};

use radicle_tui as tui;

use tui::event::Key;
use tui::store::Update;
use tui::task::EmptyProcessors;
use tui::ui::layout::Spacing;
use tui::ui::theme::Theme;
use tui::ui::widget::{Borders, Column, TableState, Window};
use tui::ui::{Context, Show, ToRow};
use tui::Channel;
use tui::Exit;

#[derive(Clone, Debug)]
struct Item {
    id: usize,
    title: String,
    timestamp: usize,
}

impl ToRow<3> for Item {
    fn to_row(&self) -> [Cell<'_>; 3] {
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
        Window::default().show(ctx, Theme::default(), |ui| {
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

                    ui.column_bar(
                        frame,
                        columns.clone(),
                        Spacing::default(),
                        Some(Borders::None),
                    );

                    let table = ui.table(
                        frame,
                        &mut selected,
                        &self.items,
                        columns,
                        None,
                        Spacing::from(1),
                        Some(Borders::None),
                    );
                    if table.changed {
                        ui.send_message(Message::SelectionChanged {
                            state: TableState::new(selected),
                        })
                    }

                    ui.shortcuts(frame, &[("q", "quit")], '|', Alignment::Left);

                    if ui.has_input(|key| key == Key::Enter) {
                        ui.send_message(Message::Return);
                    }
                },
            );

            if ui.has_input(|key| key == Key::Char('q')) {
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
                title: format!("Title of item #{id}"),
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("Could not read system time")
                    .as_secs() as usize,
            })
            .collect(),
        selector: TableState::new(Some(0)),
    };

    if let Some(exit) = tui::im(
        app,
        Viewport::Inline(12),
        Channel::default(),
        EmptyProcessors::new(),
    )
    .await?
    {
        println!("{exit}");
    } else {
        anyhow::bail!("No selection");
    }

    Ok(())
}
