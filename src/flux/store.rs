use std::marker::PhantomData;
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use super::termination::{Interrupted, Terminator};

pub trait State<A> {
    fn tick(&self);

    fn handle_action(self, action: A) -> anyhow::Result<bool>;
}

pub struct Store<A, S>
where
    S: State<A> + Clone + Send + Sync,
{
    state_tx: UnboundedSender<S>,
    _phantom: PhantomData<A>,
}

impl<A, S> Store<A, S>
where
    S: State<A> + Clone + Send + Sync,
{
    pub fn new() -> (Self, UnboundedReceiver<S>) {
        let (state_tx, state_rx) = mpsc::unbounded_channel::<S>();

        (
            Store {
                state_tx,
                _phantom: PhantomData,
            },
            state_rx,
        )
    }
}

impl<A, S> Store<A, S>
where
    S: State<A> + Clone + Send + Sync + 'static,
{
    pub async fn main_loop(
        self,
        state: S,
        mut terminator: Terminator,
        mut action_rx: UnboundedReceiver<A>,
        mut interrupt_rx: broadcast::Receiver<Interrupted>,
    ) -> anyhow::Result<Interrupted> {
        // the initial state once
        self.state_tx.send(state.clone())?;

        let mut ticker = tokio::time::interval(Duration::from_secs(1));

        let result = loop {
            tokio::select! {
                // Handle the actions coming from the UI
                // and process them to do async operations
                Some(action) = action_rx.recv() => {
                    let exit = state.clone().handle_action(action)?;
                    if exit {
                        let _ = terminator.terminate(Interrupted::UserInt);

                        break Interrupted::UserInt;
                    }
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

            self.state_tx.send(state.clone())?;
        };

        Ok(result)
    }
}
