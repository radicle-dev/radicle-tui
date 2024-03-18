use std::fmt::Debug;
use std::marker::PhantomData;
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::Exit;

use super::task::{Interrupted, Terminator};

const STORE_TICK_RATE: Duration = Duration::from_millis(1000);

pub trait State<A, P>
where
    P: Clone + Debug + Send + Sync,
{
    fn tick(&self);

    fn handle_action(&mut self, action: A) -> Option<Exit<P>>;
}

pub struct Store<A, S, P>
where
    S: State<A, P> + Clone + Send + Sync,
    P: Clone + Debug + Send + Sync,
{
    state_tx: UnboundedSender<S>,
    _phantom: PhantomData<(A, P)>,
}

impl<A, S, P> Store<A, S, P>
where
    S: State<A, P> + Clone + Send + Sync,
    P: Clone + Debug + Send + Sync,
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

impl<A, S, P> Store<A, S, P>
where
    S: State<A, P> + Clone + Debug + Send + Sync + 'static,
    P: Clone + Debug + Send + Sync + 'static,
{
    pub async fn main_loop(
        self,
        mut state: S,
        mut terminator: Terminator<P>,
        mut action_rx: UnboundedReceiver<A>,
        mut interrupt_rx: broadcast::Receiver<Interrupted<P>>,
    ) -> anyhow::Result<Interrupted<P>> {
        // the initial state once
        self.state_tx.send(state.clone())?;

        let mut ticker = tokio::time::interval(STORE_TICK_RATE);

        let result = loop {
            tokio::select! {
                // Handle the actions coming from the UI
                // and process them to do async operations
                Some(action) = action_rx.recv() => {
                    if let Some(exit) = state.handle_action(action) {
                        let interrupted = Interrupted::User { payload: exit.value };
                        let _ = terminator.terminate(interrupted.clone());

                        break interrupted;
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
