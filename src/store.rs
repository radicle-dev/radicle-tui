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

/// A `StateValue` that writes updates to an internal
/// buffer. This buffer can be applied or reset.
///
/// Reading from a `StateValue` will return the buffer if it's
/// not empty. It will return the actual value otherwise.
#[derive(Clone, Debug)]
pub struct StateValue<T>
where
    T: Clone,
{
    value: T,
    buffer: Option<T>,
}

impl<T> StateValue<T>
where
    T: Clone,
{
    pub fn new(value: T) -> Self {
        Self {
            value,
            buffer: None,
        }
    }

    pub fn apply(&mut self) {
        if let Some(buffer) = self.buffer.clone() {
            self.value = buffer;
        }
        self.buffer = None;
    }

    pub fn reset(&mut self) {
        self.buffer = None;
    }

    pub fn write(&mut self, value: T) {
        self.buffer = Some(value);
    }

    pub fn read(&self) -> T {
        if let Some(buffer) = self.buffer.clone() {
            buffer
        } else {
            self.value.clone()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn state_value_read_should_succeed() {
        let value = StateValue::new(0);
        assert_eq!(value.read(), 0);
    }

    #[test]
    fn state_value_read_buffer_should_succeed() {
        let mut value = StateValue::new(0);
        value.write(1);

        assert_eq!(value.read(), 1);
    }

    #[test]
    fn state_value_apply_should_succeed() {
        let mut value = StateValue::new(0);

        value.write(1);
        assert_eq!(value.read(), 1);

        value.apply();
        assert_eq!(value.read(), 1);
    }

    #[test]
    fn state_value_reset_should_succeed() {
        let mut value = StateValue::new(0);

        value.write(1);
        assert_eq!(value.read(), 1);

        value.reset();
        assert_eq!(value.read(), 0);
    }

    #[test]
    fn state_value_reset_after_apply_should_succeed() {
        let mut value = StateValue::new(0);

        value.write(1);
        assert_eq!(value.read(), 1);

        value.apply();
        value.reset();
        assert_eq!(value.read(), 1);
    }
}
