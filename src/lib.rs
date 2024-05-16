pub mod cob;
pub mod context;
pub mod event;
pub mod git;
pub mod log;
pub mod store;
pub mod task;
pub mod terminal;
pub mod ui;

use std::fmt::Debug;

use anyhow::Result;

use serde::ser::{Serialize, SerializeStruct, Serializer};
use store::State;
use task::Interrupted;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use ui::{widget::Widget, Frontend};

/// An optional return value.
#[derive(Clone, Debug)]
pub struct Exit<T> {
    pub value: Option<T>,
}

/// The output that is returned by all selection interfaces.
#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct Selection<I>
where
    I: ToString,
{
    pub operation: Option<String>,
    pub ids: Vec<I>,
    pub args: Vec<String>,
}

impl<I> Selection<I>
where
    I: ToString,
{
    pub fn with_operation(mut self, operation: String) -> Self {
        self.operation = Some(operation);
        self
    }

    pub fn with_id(mut self, id: I) -> Self {
        self.ids.push(id);
        self
    }

    pub fn with_args(mut self, arg: String) -> Self {
        self.args.push(arg);
        self
    }
}

impl<I> Serialize for Selection<I>
where
    I: ToString,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("", 3)?;
        state.serialize_field("operation", &self.operation)?;
        state.serialize_field(
            "ids",
            &self.ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
        )?;
        state.serialize_field("args", &self.args)?;
        state.end()
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

/// A multi-producer, single-consumer message channel.
pub struct Channel<M> {
    pub tx: UnboundedSender<M>,
    pub rx: UnboundedReceiver<M>,
}

impl<A> Default for Channel<A> {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self { tx: tx.clone(), rx }
    }
}

/// Initialize a `Store` with the `State` given and a `Frontend` with the `Widget` given,
/// and run their main loops concurrently. Connect them to the `Channel` and also to
/// an interrupt broadcast channel also initialized in this function.
pub async fn run<S, M, W, P>(channel: Channel<M>, state: S, root: W) -> Result<Option<P>>
where
    S: State<P, Message = M> + Clone + Debug + Send + Sync + 'static,
    W: Widget<State = S, Message = M>,
    P: Clone + Debug + Send + Sync + 'static,
{
    let (terminator, mut interrupt_rx) = task::create_termination();

    let (store, state_rx) = store::Store::<S, M, P>::new();
    let frontend = Frontend::<M>::new(channel.tx.clone());

    tokio::try_join!(
        store.main_loop(state, terminator, channel.rx, interrupt_rx.resubscribe()),
        frontend.main_loop(Some(root), state_rx, interrupt_rx.resubscribe()),
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
