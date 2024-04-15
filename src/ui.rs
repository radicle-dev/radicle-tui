pub mod ext;
pub mod format;
pub mod items;
pub mod layout;
pub mod span;
pub mod theme;
pub mod widget;

use std::fmt::Debug;
use std::io::{self};
use std::time::Duration;

use termion::raw::RawTerminal;

use tokio::sync::broadcast;
use tokio::sync::mpsc::{self, UnboundedReceiver};

use super::event::Event;
use super::store::State;
use super::task::Interrupted;
use super::terminal;
use super::terminal::TermionBackendExt;
use super::ui::widget::Widget;

type Backend = TermionBackendExt<RawTerminal<io::Stdout>>;

const RENDERING_TICK_RATE: Duration = Duration::from_millis(250);
const INLINE_HEIGHT: usize = 20;

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
        W: Widget<S, A, Backend>,
        P: Clone + Send + Sync + Debug,
    {
        let mut ticker = tokio::time::interval(RENDERING_TICK_RATE);

        let mut terminal = terminal::setup(INLINE_HEIGHT)?;
        let mut events_rx = terminal::events();

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
                    Event::Resize => (),
                },
                // Handle state updates
                Some(state) = state_rx.recv() => {
                    root.update(&state);
                },
                // Catch and handle interrupt signal to gracefully shutdown
                Ok(interrupted) = interrupt_rx.recv() => {
                    let size = terminal.get_frame().size();
                    let _ = terminal.set_cursor(size.x, size.y);

                    break Ok(interrupted);
                }
            }
            terminal.draw(|frame| root.render(frame, frame.size(), &()))?;
        };

        terminal::restore(&mut terminal)?;

        result
    }
}
