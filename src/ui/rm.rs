pub mod widget;

use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::mpsc::UnboundedReceiver;

use ratatui::Viewport;

use crate::event::Event;
use crate::store::Update;
use crate::terminal::Terminal;
use crate::ui::rm::widget::RenderProps;
use crate::ui::rm::widget::Widget;
use crate::Interrupted;
use crate::Share;

const RENDERING_TICK_RATE: Duration = Duration::from_millis(250);

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
    pub async fn run<S, M, R>(
        self,
        mut root: Widget<S, M>,
        mut state_rx: UnboundedReceiver<S>,
        mut events_rx: UnboundedReceiver<Event>,
        mut interrupt_rx: broadcast::Receiver<Interrupted<R>>,
        viewport: Viewport,
    ) -> anyhow::Result<Interrupted<R>>
    where
        S: Update<M, Return = R> + 'static,
        M: Share,
        R: Share,
    {
        let mut ticker = tokio::time::interval(RENDERING_TICK_RATE);
        let mut terminal = Terminal::try_from(viewport)?;
        let mut root = {
            let state = state_rx.recv().await.unwrap();

            root.init();
            root.update(&state);
            root
        };

        let result: anyhow::Result<Interrupted<R>> = loop {
            tokio::select! {
                // Tick to terminate the select every N milliseconds
                _ = ticker.tick() => (),
                // Handle input events
                Some(event) = events_rx.recv() => match event {
                    Event::Key(key) => root.handle_event(key),
                    Event::Resize => {
                        log::info!("Resizing frontend...");
                    },
                },
                // Handle state updates
                Some(state) = state_rx.recv() => {
                    root.update(&state);
                },
                // Catch and handle interrupt signal to gracefully shutdown
                Ok(interrupted) = interrupt_rx.recv() => {
                    terminal.restore()?;

                    break Ok(interrupted);
                }
            }
            terminal.draw(|frame| root.render(RenderProps::from(frame.area()), frame))?;
        };
        terminal.restore()?;

        result
    }
}
