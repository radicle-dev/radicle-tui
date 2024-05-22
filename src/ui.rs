pub mod ext;
pub mod format;
pub mod items;
pub mod layout;
pub mod span;
pub mod theme;
pub mod widget;

use std::fmt::Debug;
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::ui::widget::RenderProps;

use self::widget::Widget;

use super::event::Event;
use super::store::State;
use super::task::Interrupted;
use super::terminal;

const RENDERING_TICK_RATE: Duration = Duration::from_millis(250);
const INLINE_HEIGHT: usize = 20;

/// The `Frontend` runs an applications' view concurrently. It handles
/// terminal events as well as state updates and renders the view accordingly.
///
/// Once created and run with `main_loop`, the `Frontend` will wait for new messages
/// being sent on either the terminal event, the state or the interrupt message channel.
#[derive(Default)]
pub struct Frontend {}

impl Frontend {
    /// By calling `main_loop`, the `Frontend` will wait for new messages being sent
    /// on either the terminal event, the state or the interrupt message channel.
    /// After all, it will draw the (potentially) updated root widget.
    ///
    /// Terminal event messages are being sent by a thread polling `stdin` for new user input
    /// and another thread polling UNIX signals, e.g. `SIGWINCH` when the terminal
    /// window size is being changed. Terminal events are then passed to the root widget
    /// of the application.
    ///
    /// State messages are being sent by the applications' `Store`. Received state updates
    /// will be passed to the root widget as well.
    ///
    /// Interrupt messages are being sent to broadcast channel for retrieving the
    /// application kill signal.
    pub async fn main_loop<S, M, P>(
        self,
        mut root: Widget<S, M>,
        mut state_rx: UnboundedReceiver<S>,
        mut interrupt_rx: broadcast::Receiver<Interrupted<P>>,
    ) -> anyhow::Result<Interrupted<P>>
    where
        S: State<P> + 'static,
        M: 'static,
        P: Clone + Send + Sync + Debug,
    {
        let mut ticker = tokio::time::interval(RENDERING_TICK_RATE);

        let mut terminal = terminal::setup(INLINE_HEIGHT)?;
        let mut events_rx = terminal::events();

        let mut root = {
            let state = state_rx.recv().await.unwrap();

            root.update(&state);
            root
        };

        let result: anyhow::Result<Interrupted<P>> = loop {
            tokio::select! {
                // Tick to terminate the select every N milliseconds
                _ = ticker.tick() => (),
                Some(event) = events_rx.recv() => match event {
                    Event::Key(key) => root.handle_event(key),
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
            terminal.draw(|frame| root.render(frame, RenderProps::from(frame.size())))?;
        };

        terminal::restore(&mut terminal)?;

        result
    }
}
