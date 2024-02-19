pub mod cob;
pub mod ext;
pub mod format;
pub mod layout;
pub mod span;
pub mod theme;
pub mod widget;

use std::io::{self};
use std::thread;
use std::time::Duration;

use termion::event::Event;
use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};

use ratatui::prelude::*;

use tokio::sync::broadcast;
use tokio::sync::mpsc::{self, UnboundedReceiver};

use super::store::State;
use super::termination::Interrupted;
use super::ui::widget::{Render, Widget};

type Backend = TermionBackend<RawTerminal<io::Stdout>>;

const RENDERING_TICK_RATE: Duration = Duration::from_millis(250);

pub struct Frontend<A> {
    action_tx: mpsc::UnboundedSender<A>,
}

impl<A> Frontend<A> {
    pub fn new() -> (Self, UnboundedReceiver<A>) {
        let (action_tx, action_rx) = mpsc::unbounded_channel();

        (Self { action_tx }, action_rx)
    }

    pub async fn main_loop<S: State<A>, W: Widget<S, A> + Render<()>>(
        self,
        mut state_rx: UnboundedReceiver<S>,
        mut interrupt_rx: broadcast::Receiver<Interrupted>,
    ) -> anyhow::Result<Interrupted> {
        let mut terminal = setup_terminal()?;
        let mut ticker = tokio::time::interval(RENDERING_TICK_RATE);
        let mut events_rx = events();

        let mut root = {
            let state = state_rx.recv().await.unwrap();

            W::new(&state, self.action_tx.clone())
        };

        // let mut last_frame: Option<CompletedFrame> = None;
        let result: anyhow::Result<Interrupted> = loop {
            tokio::select! {
                // Tick to terminate the select every N milliseconds
                _ = ticker.tick() => (),
                Some(event) = events_rx.recv() => if let Event::Key(key) = event {
                    root.handle_key_event(key)
                },
                // Handle state updates
                Some(state) = state_rx.recv() => {
                    root = root.move_with_state(&state);
                },
                // Catch and handle interrupt signal to gracefully shutdown
                Ok(interrupted) = interrupt_rx.recv() => {
                    let size = terminal.get_frame().size();
                    // terminal.set_cursor(size.width, size.height + size.y)?;
                    terminal.set_cursor(size.x, size.y)?;

                    break Ok(interrupted);
                }
            }
            terminal.draw(|frame| root.render::<Backend>(frame, frame.size(), ()))?;
        };

        restore_terminal(&mut terminal)?;

        result
    }
}

fn setup_terminal() -> anyhow::Result<Terminal<Backend>> {
    let stdout = io::stdout().into_raw_mode()?;
    let options = TerminalOptions {
        viewport: Viewport::Inline(20),
    };

    Ok(Terminal::with_options(
        TermionBackend::new(stdout),
        options,
    )?)
}

fn restore_terminal(_terminal: &mut Terminal<Backend>) -> anyhow::Result<()> {
    Ok(())
}

fn events() -> mpsc::UnboundedReceiver<Event> {
    let (tx, rx) = mpsc::unbounded_channel();
    let keys_tx = tx.clone();
    thread::spawn(move || {
        let stdin = io::stdin();
        for key in stdin.keys().flatten() {
            if keys_tx.send(Event::Key(key)).is_err() {
                return;
            }
        }
    });
    rx
}
