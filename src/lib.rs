pub mod event;
pub mod store;
pub mod task;
pub mod terminal;
pub mod ui;

use std::any::Any;
use std::fmt::Debug;

use ratatui::Viewport;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use serde::ser::{Serialize, SerializeStruct, Serializer};

use anyhow::Result;

use store::Update;
use task::Interrupted;
use ui::im;
use ui::im::Show;
use ui::rm;

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
pub async fn rm<S, M, P>(
    state: S,
    root: rm::widget::Widget<S, M>,
    viewport: Viewport,
    channel: Channel<M>,
) -> Result<Option<P>>
where
    S: Update<M, Return = P> + Clone + Debug + Send + Sync + 'static,
    M: Debug + Send + Sync + 'static,
    P: Clone + Debug + Send + Sync + 'static,
{
    let (terminator, mut interrupt_rx) = task::create_termination();

    let (store, state_rx) = store::Store::<S, M, P>::new();
    let frontend = rm::Frontend::default();

    tokio::try_join!(
        store.run(state, terminator, channel.rx, interrupt_rx.resubscribe()),
        frontend.run(root, state_rx, interrupt_rx.resubscribe(), viewport),
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

/// Initialize a `Store` with the `State` given and a `Frontend` with the `App` given,
/// and run their main loops concurrently. Connect them to the `Channel` and also to
/// an interrupt broadcast channel also initialized in this function.
pub async fn im<S, M, P>(state: S, viewport: Viewport, channel: Channel<M>) -> Result<Option<P>>
where
    S: Update<M, Return = P> + Show<M> + Clone + Send + Sync + 'static,
    M: Clone + Debug + Send + Sync + 'static,
    P: Clone + Debug + Send + Sync + 'static,
{
    let (terminator, mut interrupt_rx) = task::create_termination();

    let state_tx = channel.tx.clone();
    let (store, state_rx) = store::Store::<S, M, P>::new();
    let frontend = im::Frontend::default();

    tokio::try_join!(
        store.run(state, terminator, channel.rx, interrupt_rx.resubscribe()),
        frontend.run(state_tx, state_rx, interrupt_rx.resubscribe(), viewport),
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
