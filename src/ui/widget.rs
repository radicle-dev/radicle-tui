pub mod container;
pub mod input;
pub mod list;
pub mod utils;
pub mod window;

use std::any::Any;
use std::rc::Rc;

use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use ratatui::prelude::*;

use self::input::{TextAreaState, TextViewState};

pub type BoxedView<S, M> = Box<dyn View<State = S, Message = M>>;
pub type UpdateCallback<S> = fn(&S) -> ViewProps;
pub type EventCallback<M> = fn(Key, Option<&ViewState>, Option<&ViewProps>) -> Option<M>;
pub type RenderCallback<M> = fn(Option<&ViewProps>, &RenderProps) -> Option<M>;

/// `ViewProps` are properties of a `View`. They define a `View`s data, configuration etc.
/// Since the framework itself does not know the concrete type of `View`, it also does not
/// know the concrete type of a `View`s properties.
/// Hence, view properties are stored inside a `Box<dyn Any>` and downcasted to the concrete
/// type when needed.
pub struct ViewProps {
    inner: Box<dyn Any>,
}

impl ViewProps {
    pub fn inner<T>(self) -> Option<T>
    where
        T: Default + Clone + 'static,
    {
        self.inner.downcast::<T>().ok().map(|inner| *inner)
    }

    pub fn inner_ref<T>(&self) -> Option<&T>
    where
        T: Default + Clone + 'static,
    {
        self.inner.downcast_ref::<T>()
    }
}

impl From<Box<dyn Any>> for ViewProps {
    fn from(props: Box<dyn Any>) -> Self {
        ViewProps { inner: props }
    }
}

impl From<&'static dyn Any> for ViewProps {
    fn from(inner: &'static dyn Any) -> Self {
        Self {
            inner: Box::new(inner),
        }
    }
}

/// A `ViewState` is the representation of a `View`s internal state. e.g. current
/// table selection or contents of a text field.
#[derive(Debug)]
pub enum ViewState {
    USize(usize),
    String(String),
    Table { selected: usize, scroll: usize },
    Tree(Vec<String>),
    TextView(TextViewState),
    TextArea(TextAreaState),
}

impl ViewState {
    pub fn unwrap_usize(&self) -> Option<usize> {
        match self {
            ViewState::USize(value) => Some(*value),
            _ => None,
        }
    }

    pub fn unwrap_string(&self) -> Option<String> {
        match self {
            ViewState::String(value) => Some(value.clone()),
            _ => None,
        }
    }

    pub fn unwrap_table(&self) -> Option<(usize, usize)> {
        match self {
            ViewState::Table { selected, scroll } => Some((*selected, *scroll)),
            _ => None,
        }
    }

    pub fn unwrap_textview(&self) -> Option<TextViewState> {
        match self {
            ViewState::TextView(state) => Some(state.clone()),
            _ => None,
        }
    }

    pub fn unwrap_textarea(&self) -> Option<TextAreaState> {
        match self {
            ViewState::TextArea(state) => Some(state.clone()),
            _ => None,
        }
    }

    pub fn unwrap_tree(&self) -> Option<Vec<String>> {
        match self {
            ViewState::Tree(value) => Some(value.clone().to_vec()),
            _ => None,
        }
    }
}

#[derive(Clone, Default)]
pub enum PredefinedLayout {
    #[default]
    None,
    Expandable3,
}

impl PredefinedLayout {
    pub fn split(&self, area: Rect) -> Rc<[Rect]> {
        match self {
            Self::Expandable3 => {
                if area.width <= 140 {
                    let [left, right] = Layout::horizontal([
                        Constraint::Percentage(50),
                        Constraint::Percentage(50),
                    ])
                    .areas(area);
                    let [right_top, right_bottom] =
                        Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)])
                            .areas(right);

                    [left, right_top, right_bottom].into()
                } else {
                    Layout::horizontal([
                        Constraint::Percentage(33),
                        Constraint::Percentage(33),
                        Constraint::Percentage(33),
                    ])
                    .split(area)
                }
            }
            _ => Layout::default().split(area),
        }
    }
}

/// General properties that specify how a `View` is rendered.
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

/// Main trait defining a `View` behaviour, which needs be implemented in order to
/// build a custom widget. A `View` operates on an application state and can emit
/// application messages. It's usually is accompanied by a definition of view-specific
/// properties, which are being built from the application state by the framework.
pub trait View {
    type State;
    type Message;

    /// Should return the internal state.
    fn view_state(&self) -> Option<ViewState> {
        None
    }

    /// Should reset the internal state and call `reset` on all children.
    fn reset(&mut self) {}

    /// Should handle key events and call `handle_event` on all children.
    fn handle_event(&mut self, _props: Option<&ViewProps>, _key: Key) -> Option<Self::Message> {
        None
    }

    /// Should update the internal props of this and all children.
    fn update(&mut self, _props: Option<&ViewProps>, _state: &Self::State) {}

    /// Should render the view using the given `RenderProps`.
    fn render(&mut self, props: Option<&ViewProps>, render: RenderProps, frame: &mut Frame);
}

/// A `View` needs to wrapped into a `Widget` before being able to use with the
/// framework. A `Widget` enhances a `View` with event and update callbacks and takes
/// care of calling them before / after calling into the `View`.
pub struct Widget<S, M> {
    view: BoxedView<S, M>,
    props: Option<ViewProps>,
    sender: UnboundedSender<M>,
    on_update: Option<UpdateCallback<S>>,
    on_event: Option<EventCallback<M>>,
    on_render: Option<RenderCallback<M>>,
}

impl<S: 'static, M: 'static> Widget<S, M> {
    pub fn new<V>(view: V, sender: UnboundedSender<M>) -> Self
    where
        Self: Sized,
        V: View<State = S, Message = M> + 'static,
    {
        Self {
            view: Box::new(view),
            props: None,
            sender: sender.clone(),
            on_update: None,
            on_event: None,
            on_render: None,
        }
    }

    /// Calls `reset` on the wrapped view.
    pub fn reset(&mut self) {
        self.view.reset()
    }

    /// Calls `handle_event` on the wrapped view as well as the `on_event` callback.
    /// Sends any message returned by either the view or the callback.
    pub fn handle_event(&mut self, key: Key) {
        if let Some(message) = self.view.handle_event(self.props.as_ref(), key) {
            let _ = self.sender.send(message);
        }

        if let Some(on_event) = self.on_event {
            if let Some(message) =
                (on_event)(key, self.view.view_state().as_ref(), self.props.as_ref())
            {
                let _ = self.sender.send(message);
            }
        }
    }

    /// Applications are usually defined by app-specific widgets that do know
    /// the type of `state`. These can use widgets from the library that do not know the
    /// type of `state`.
    ///
    /// If `on_update` is set, implementations of this function should call it to
    /// construct and update the internal props. If it is not set, app widgets can construct
    /// props directly via their state converters, whereas library widgets can just fallback
    /// to their current props.
    pub fn update(&mut self, state: &S) {
        self.props = self.on_update.map(|on_update| (on_update)(state));
        self.view.update(self.props.as_ref(), state);
    }

    /// Renders the wrapped view.
    pub fn render(&mut self, render: RenderProps, frame: &mut Frame) {
        self.view.render(self.props.as_ref(), render.clone(), frame);

        if let Some(on_render) = self.on_render {
            (on_render)(self.props.as_ref(), &render)
                .and_then(|message| self.sender.send(message).ok());
        }
    }

    /// Sets the optional custom event handler.
    pub fn on_event(mut self, callback: EventCallback<M>) -> Self
    where
        Self: Sized,
    {
        self.on_event = Some(callback);
        self
    }

    /// Sets the optional update handler.
    pub fn on_update(mut self, callback: UpdateCallback<S>) -> Self
    where
        Self: Sized,
    {
        self.on_update = Some(callback);
        self
    }

    /// Sets the optional update handler.
    pub fn on_render(mut self, callback: RenderCallback<M>) -> Self
    where
        Self: Sized,
    {
        self.on_render = Some(callback);
        self
    }
}

/// A `View` needs to be wrapped into a `Widget` in order to be used with the framework.
/// `ToWidget` provides a blanket implementation for all `View`s.
pub trait ToWidget<S, M> {
    fn to_widget(self, tx: UnboundedSender<M>) -> Widget<S, M>
    where
        Self: Sized + 'static;
}

impl<T, S, M> ToWidget<S, M> for T
where
    T: View<State = S, Message = M>,
    S: 'static,
    M: 'static,
{
    fn to_widget(self, tx: UnboundedSender<M>) -> Widget<S, M>
    where
        Self: Sized + 'static,
    {
        Widget::new(self, tx)
    }
}
