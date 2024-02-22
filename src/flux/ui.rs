pub mod cob;
pub mod ext;
pub mod format;
pub mod layout;
pub mod span;
pub mod theme;
pub mod widget;

use std::fmt::Debug;
use std::io::{self};
use std::thread;
use std::time::Duration;

// use termion::event::Event;
use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};

use ratatui::prelude::*;

use tokio::sync::broadcast;
use tokio::sync::mpsc::{self, UnboundedReceiver};

use super::event::Event;
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

    pub async fn main_loop<S, W, P>(
        self,
        mut state_rx: UnboundedReceiver<S>,
        mut interrupt_rx: broadcast::Receiver<Interrupted<P>>,
    ) -> anyhow::Result<Interrupted<P>>
    where
        S: State<A, P>,
        W: Widget<S, A> + Render<()>,
        P: Clone + Send + Sync + Debug,
    {
        let mut terminal = setup_terminal()?;
        let mut ticker = tokio::time::interval(RENDERING_TICK_RATE);
        let mut events_rx = events();

        let mut root = {
            let state = state_rx.recv().await.unwrap();

            W::new(&state, self.action_tx.clone())
        };

        let result: anyhow::Result<Interrupted<P>> = loop {
            tokio::select! {
                // Tick to terminate the select every N milliseconds
                _ = ticker.tick() => (),
                Some(event) = events_rx.recv() => match event {
                    Event::Key(key) => root.handle_key_event(key),
                    Event::Resize => println!("here"),
                },
                // Handle state updates
                Some(state) = state_rx.recv() => {
                    root = root.move_with_state(&state);
                },
                // Catch and handle interrupt signal to gracefully shutdown
                Ok(interrupted) = interrupt_rx.recv() => {
                    let size = terminal.get_frame().size();
                    let _ = terminal.set_cursor(size.x, size.y);

                    break Ok(interrupted);
                }
            }
            let _ = terminal.draw(|frame| root.render::<Backend>(frame, frame.size(), ()));
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

fn restore_terminal(terminal: &mut Terminal<Backend>) -> anyhow::Result<()> {
    terminal.clear()?;
    Ok(())
}

fn events() -> mpsc::UnboundedReceiver<Event> {
    let (tx, rx) = mpsc::unbounded_channel();
    let events_tx = tx.clone();
    thread::spawn(move || {
        let stdin = io::stdin();
        for key in stdin.keys().flatten() {
            if events_tx.send(Event::Key(key)).is_err() {
                return;
            }
        }
    });

    let events_tx = tx.clone();
    if let Ok(mut signals) = signal_hook::iterator::Signals::new(&[libc::SIGWINCH]) {
        thread::spawn(move || {
            for signal in signals.forever() {
                if events_tx.send(Event::Resize).is_err() {
                    return;
                }
            }
        });
    }
    rx
}
