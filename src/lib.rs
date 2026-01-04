pub mod event;
pub mod store;
pub mod task;
pub mod terminal;
pub mod ui;

use std::any::Any;
use std::fmt::Debug;

use anyhow::Result;

use serde::ser::{Serialize, SerializeStruct, Serializer};

#[cfg(unix)]
use tokio::signal::unix::signal;
use tokio::sync::broadcast;
use tokio::sync::mpsc::unbounded_channel;

use ratatui::Viewport;

use store::Update;
use terminal::StdinReader;
use ui::{Frontend, Show};

use crate::task::Process;

/// An optional return value.
#[derive(Clone, Debug)]
pub struct Exit<T> {
    pub value: Option<T>,
}

/// The output that is returned by all selection interfaces.
#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct Selection<O>
where
    O: Serialize,
{
    pub operation: Option<O>,
    pub args: Vec<String>,
}

impl<O> Selection<O>
where
    O: Serialize,
{
    pub fn with_operation(mut self, operation: O) -> Self {
        self.operation = Some(operation);
        self
    }

    pub fn with_args(mut self, arg: String) -> Self {
        self.args.push(arg);
        self
    }
}

impl<O> Serialize for Selection<O>
where
    O: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("", 3)?;
        state.serialize_field("operation", &self.operation)?;
        state.serialize_field("args", &self.args)?;
        state.end()
    }
}

/// Implementors of `Share` can be used inside the multi-threaded
/// application environment.
pub trait Share: Clone + Debug + Send + Sync + 'static {}

/// Blanket implementation for all types that implement the required
/// traits.
impl<T: Clone + Debug + Send + Sync + 'static> Share for T {}

/// Provide implementations for conversions to and from `Box<dyn Any>`.
pub trait BoxedAny {
    fn from_boxed_any(any: Box<dyn Any>) -> Option<Self>
    where
        Self: Sized + Clone + 'static;

    fn to_boxed_any(self) -> Box<dyn Any>
    where
        Self: Sized + Clone + 'static;
}

impl<T> BoxedAny for T
where
    T: Sized + Clone + 'static,
{
    fn from_boxed_any(any: Box<dyn Any>) -> Option<Self>
    where
        Self: Sized + Clone + 'static,
    {
        any.downcast::<Self>().ok().map(|b| *b)
    }

    fn to_boxed_any(self) -> Box<dyn Any>
    where
        Self: Sized + Clone + 'static,
    {
        Box::new(self)
    }
}

/// A 'PageStack' for applications. Page identifier can be pushed to and
/// popped from the stack.
#[derive(Clone, Default, Debug)]
pub struct PageStack<T> {
    pages: Vec<T>,
}

impl<T> PageStack<T> {
    pub fn new(pages: Vec<T>) -> Self {
        Self { pages }
    }

    pub fn push(&mut self, page: T) {
        self.pages.push(page);
    }

    pub fn pop(&mut self) -> Option<T> {
        self.pages.pop()
    }

    pub fn peek(&self) -> Result<&T> {
        match self.pages.last() {
            Some(page) => Ok(page),
            None => Err(anyhow::anyhow!(
                "Could not peek active page. Page stack is empty."
            )),
        }
    }
}

/// A multi-producer, multi-consumer message channel.
pub struct Channel<M> {
    pub tx: broadcast::Sender<M>,
    pub rx: broadcast::Receiver<M>,
}

impl<M: Clone> Default for Channel<M> {
    fn default() -> Self {
        let (tx, rx) = broadcast::channel(1000);
        Self { tx, rx }
    }
}

/// Initialize a `Store` with the `State` given and a `Frontend` with the `App` given,
/// and run their main loops concurrently. Connect them to the `Channel` and also to
/// an interrupt broadcast channel also initialized in this function.
/// Additionally, a list of processors can be passed. Processors will also receive all
/// applications messages and can emit new ones. They will be executed by an internal worker.
pub async fn im<S, T, M, R>(
    state: S,
    viewport: Viewport,
    channel: Channel<M>,
    processors: Vec<T>,
) -> Result<Option<R>>
where
    S: Update<M, Return = R> + Show<M> + Share,
    T: Process<M> + Share,
    M: Share,
    R: Share,
{
    let (terminator, mut interrupt_rx) = create_termination();
    let (state_tx, state_rx) = unbounded_channel();
    let (event_tx, event_rx) = unbounded_channel();
    let (work_tx, work_rx) = unbounded_channel();

    let store = store::Store::<S, M, R>::new(state_tx.clone());
    let worker = task::Worker::<T, M, R>::new(work_tx.clone());
    let frontend = Frontend::default();
    let stdin_reader = StdinReader::default();

    // TODO(erikli): Handle errors
    let _ = tokio::try_join!(
        worker.run(
            processors,
            channel.rx.resubscribe(),
            interrupt_rx.resubscribe()
        ),
        store.run(
            state,
            terminator,
            channel.rx.resubscribe(),
            work_rx,
            interrupt_rx.resubscribe(),
        ),
        frontend.run(
            channel.tx,
            state_rx,
            event_rx,
            interrupt_rx.resubscribe(),
            viewport
        ),
        stdin_reader.run(event_tx, interrupt_rx.resubscribe()),
    )?;

    if let Ok(reason) = interrupt_rx.recv().await {
        match reason {
            Interrupted::User { payload } => Ok(payload),
            Interrupted::OsSignal => anyhow::bail!("exited because of an os sig int"),
        }
    } else {
        anyhow::bail!("exited because of an unexpected error");
    }
}

/// An `Interrupt` message that is produced by either an OS signal (e.g. kill)
/// or the user by requesting the application to close.
#[derive(Debug, Clone)]
pub enum Interrupted<P>
where
    P: Share,
{
    OsSignal,
    User { payload: Option<P> },
}

/// The `Terminator` wraps a broadcast channel and can send an interrupt messages.
#[derive(Debug, Clone)]
pub struct Terminator<P>
where
    P: Share,
{
    interrupt_tx: broadcast::Sender<Interrupted<P>>,
}

impl<P> Terminator<P>
where
    P: Share,
{
    /// Create a `Terminator` that stores the sending end of a broadcast channel.
    pub fn new(interrupt_tx: broadcast::Sender<Interrupted<P>>) -> Self {
        Self { interrupt_tx }
    }

    /// Send interrupt message to the broadcast channel.
    pub fn terminate(&mut self, interrupted: Interrupted<P>) -> anyhow::Result<()> {
        self.interrupt_tx.send(interrupted)?;

        Ok(())
    }
}

/// Receive `SIGINT` and call terminator which sends the interrupt message to its broadcast channel.
#[cfg(unix)]
async fn terminate_by_unix_signal<P>(mut terminator: Terminator<P>)
where
    P: Share,
{
    let mut interrupt_signal = signal(tokio::signal::unix::SignalKind::interrupt())
        .expect("failed to create interrupt signal stream");

    interrupt_signal.recv().await;

    terminator
        .terminate(Interrupted::OsSignal)
        .expect("failed to send interrupt signal");
}

/// Create a broadcast channel and spawn a task for retrieving the applications' kill signal.
pub fn create_termination<P>() -> (Terminator<P>, broadcast::Receiver<Interrupted<P>>)
where
    P: Share,
{
    let (tx, rx) = broadcast::channel(1);
    let terminator = Terminator::new(tx);

    #[cfg(unix)]
    tokio::spawn(terminate_by_unix_signal(terminator.clone()));

    (terminator, rx)
}
