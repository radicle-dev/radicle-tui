pub mod container;
pub mod input;
pub mod list;
pub mod text;
pub mod window;

use std::any::Any;

use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::prelude::*;
use ratatui::widgets::Cell;

pub type BoxedWidget<S, M> = Box<dyn Widget<State = S, Message = M>>;

pub type UpdateCallback<S> = fn(&S) -> Box<dyn Any>;
pub type EventCallback = fn(&mut dyn Any);

/// A `WidgetBase` provides common functionality to a `Widget`. It's used to store
/// event and update callbacks as well sending messages to the UI's message channel.
pub struct WidgetBase<S, M> {
    /// Message sender
    pub tx: UnboundedSender<M>,
    /// Custom update handler
    pub on_update: Option<UpdateCallback<S>>,
    /// Additional custom event handler
    pub on_event: Option<EventCallback>,
}

impl<S, M> WidgetBase<S, M> {
    /// Create a new `WidgetBase` with no callbacks set.
    pub fn new(tx: UnboundedSender<M>) -> Self {
        Self {
            tx: tx.clone(),
            on_update: None,
            on_event: None,
        }
    }

    /// Send a message to the internal channel.
    pub fn send(&self, message: M) -> Result<(), SendError<M>> {
        self.tx.send(message)
    }
}

/// General properties that specify how a `Widget` is rendered.
/// They can be passed to a widgets' `render` function.
#[derive(Clone, Default)]
pub struct RenderProps {
    /// Area of the render props.
    pub area: Rect,
    /// Layout to be rendered in.
    pub layout: Layout,
    /// Focus of the render props.
    pub focus: bool,
}

impl RenderProps {
    /// Sets the focus of these render props.
    pub fn focus(mut self, focus: bool) -> Self {
        self.focus = focus;
        self
    }

    /// Sets the layout of these render props.
    pub fn layout(mut self, layout: Layout) -> Self {
        self.layout = layout;
        self
    }
}

impl From<Rect> for RenderProps {
    fn from(area: Rect) -> Self {
        Self {
            area,
            layout: Layout::default(),
            focus: false,
        }
    }
}

/// Main trait defining a `Widget` behaviour.
///
/// This is the trait that you should implement to define a custom `Widget`.
pub trait Widget {
    type State;
    type Message;

    /// Should return a new view with props build from state (if type is known) and a
    /// message sender set.
    fn new(state: &Self::State, tx: UnboundedSender<Self::Message>) -> Self
    where
        Self: Sized;

    /// Should handle key events and call `handle_event` on all children.
    ///
    /// After key events have been handled, the custom event handler `on_event` should
    /// be called
    fn handle_event(&mut self, key: Key);

    /// Should update the internal props of this and all children.
    ///
    /// Applications are usually defined by app-specific widgets that do know
    /// the type of `state`. These can use widgets from the library that do not know the
    /// type of `state`.
    ///
    /// If `on_update` is set, implementations of this function should call it to
    /// construct and update the internal props. If it is not set, app widgets can construct
    /// props directly via their state converters, whereas library widgets can just fallback
    /// to their current props.
    fn update(&mut self, state: &Self::State);

    /// Renders a widget to the given frame in the given area.
    ///
    /// Optional render props can be given.
    fn render(&self, frame: &mut Frame, props: RenderProps);

    /// Return a reference to this widgets' base.
    fn base(&self) -> &WidgetBase<Self::State, Self::Message>;

    /// Return a mutable reference to this widgets' base.
    fn base_mut(&mut self) -> &mut WidgetBase<Self::State, Self::Message>;

    /// Send a message to the widgets' base channel.
    fn send(&self, message: Self::Message) -> Result<(), SendError<Self::Message>> {
        self.base().send(message)
    }

    /// Should set the optional custom event handler.
    fn on_event(mut self, callback: EventCallback) -> Self
    where
        Self: Sized,
    {
        self.base_mut().on_event = Some(callback);
        self
    }

    /// Should set the optional update handler.
    fn on_update(mut self, callback: UpdateCallback<Self::State>) -> Self
    where
        Self: Sized,
    {
        self.base_mut().on_update = Some(callback);
        self
    }

    /// Returns a boxed `Widget`
    fn to_boxed(self) -> Box<Self>
    where
        Self: Sized,
    {
        Box::new(self)
    }
}

/// Needs to be implemented for items that are supposed to be rendered in tables.
pub trait ToRow<const W: usize> {
    fn to_row(&self) -> [Cell; W];
}

/// Common trait for widget properties.
pub trait Properties {
    fn to_boxed(self) -> Box<Self>
    where
        Self: Sized,
    {
        Box::new(self)
    }

    fn from_callback<S>(callback: Option<UpdateCallback<S>>, state: &S) -> Option<Self>
    where
        Self: Sized + Clone + 'static + BoxedAny,
    {
        callback
            .map(|callback| (callback)(state))
            .and_then(|props| Self::from_boxed_any(props))
    }
}

/// Provide default implementations for conversions to and from `Box<dyn Any>`.
pub trait BoxedAny {
    fn from_boxed_any(any: Box<dyn Any>) -> Option<Self>
    where
        Self: Sized + Clone + 'static,
    {
        any.downcast_ref::<Self>().cloned()
    }

    fn to_boxed_any(self) -> Box<dyn Any>
    where
        Self: Sized + Clone + 'static,
    {
        Box::new(self)
    }
}
