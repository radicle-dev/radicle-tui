use std::marker::PhantomData;
use std::time::Duration;

use tokio::sync::{
    broadcast,
    mpsc::{UnboundedReceiver, UnboundedSender},
};

use super::{Exit, Interrupted, Share, Terminator};

const STORE_TICK_RATE: Duration = Duration::from_millis(1000);

/// The main state trait for the ability to update an applications' state.
/// Implementations should handle user-defined application messages as well as ticks.
pub trait Update<M> {
    type Return;

    /// Handle a user-defined application message and return an `Exit` object
    /// in case the received message requested the application to also quit.
    fn update(&mut self, message: M) -> Option<Exit<Self::Return>>;

    /// Handle recurring tick.
    fn tick(&mut self) {}
}

/// The `Store` updates the applications' state concurrently. It handles
/// messages coming from the frontend and updates the state accordingly.
pub struct Store<S, M, R>
where
    S: Update<M, Return = R> + Share,
{
    state_tx: UnboundedSender<S>,
    _phantom: PhantomData<(M, R)>,
}

impl<S, M, R> Store<S, M, R>
where
    S: Update<M, Return = R> + Share,
    R: Share,
{
    pub fn new(tx: UnboundedSender<S>) -> Self {
        Self {
            state_tx: tx,
            _phantom: PhantomData,
        }
    }
}

impl<S, M, R> Store<S, M, R>
where
    S: Update<M, Return = R> + Share,
    M: Share,
    R: Share,
{
    /// By calling `main_loop`, the store will wait for new messages coming
    /// from the frontend and update the applications' state accordingly. It will
    /// also tick with the defined `STORE_TICK_RATE`.
    /// Updated states are then being send to the state message channel.
    pub async fn run(
        self,
        mut state: S,
        mut terminator: Terminator<R>,
        mut message_rx: broadcast::Receiver<M>,
        mut work_rx: UnboundedReceiver<M>,
        mut interrupt_rx: broadcast::Receiver<Interrupted<R>>,
    ) -> anyhow::Result<Interrupted<R>> {
        // Send the initial state once
        self.state_tx.send(state.clone())?;

        let mut ticker = tokio::time::interval(STORE_TICK_RATE);

        let result = loop {
            tokio::select! {
                // Handle the messages coming from the frontend
                // and process them to do async operations
                Ok(message) = message_rx.recv() => {
                    if let Some(exit) = state.update(message) {
                        let interrupted = Interrupted::User { payload: exit.value };
                        let _ = terminator.terminate(interrupted.clone());

                        break interrupted;
                    }
                    self.state_tx.send(state.clone())?;
                },
                Some(message) = work_rx.recv() => {
                    state.update(message);
                    self.state_tx.send(state.clone())?;
                },
                // Tick to terminate the select every N milliseconds
                _ = ticker.tick() => {
                    state.tick();
                },
                // Catch and handle interrupt signal to gracefully shutdown
                Ok(interrupted) = interrupt_rx.recv() => {
                    break interrupted;
                }
            }
        };

        Ok(result)
    }
}
