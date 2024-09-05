use std::fmt::Debug;
use std::marker::PhantomData;
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::Exit;

use super::task::{Interrupted, Terminator};

const STORE_TICK_RATE: Duration = Duration::from_millis(1000);

/// The `State` known to the application store. It handles user-defined
/// application messages as well as ticks.
pub trait State<P>
where
    P: Clone + Debug + Send + Sync,
{
    type Message;

    /// Handle a user-defined application message and return an `Exit` object
    /// in case the received message requested the application to also quit.
    fn update(&mut self, message: Self::Message) -> Option<Exit<P>>;

    /// Handle recurring tick.
    fn tick(&mut self) {}
}

/// The `Store` updates the applications' state concurrently. It handles
/// messages coming from the frontend and updates the state accordingly.
pub struct Store<S, M, P>
where
    S: State<P> + Clone + Send + Sync,
    P: Clone + Debug + Send + Sync,
{
    state_tx: UnboundedSender<S>,
    _phantom: PhantomData<(M, P)>,
}

impl<S, M, P> Store<S, M, P>
where
    S: State<P> + Clone + Send + Sync,
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

impl<S, M, P> Store<S, M, P>
where
    S: State<P, Message = M> + Clone + Debug + Send + Sync + 'static,
    P: Clone + Debug + Send + Sync + 'static,
{
    /// By calling `main_loop`, the store will wait for new messages coming
    /// from the frontend and update the applications' state accordingly. It will
    /// also tick with the defined `STORE_TICK_RATE`.
    /// Updated states are then being send to the state message channel.
    pub async fn run(
        self,
        mut state: S,
        mut terminator: Terminator<P>,
        mut message_rx: UnboundedReceiver<M>,
        mut interrupt_rx: broadcast::Receiver<Interrupted<P>>,
    ) -> anyhow::Result<Interrupted<P>> {
        // Send the initial state once
        self.state_tx.send(state.clone())?;

        let mut ticker = tokio::time::interval(STORE_TICK_RATE);

        let result = loop {
            tokio::select! {
                // Handle the messages coming from the frontend
                // and process them to do async operations
                Some(message) = message_rx.recv() => {
                    if let Some(exit) = state.update(message) {
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
