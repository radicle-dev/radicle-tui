use std::hash::Hash;
use std::time::Duration;

use anyhow::Result;
use serde::ser::{Serialize, SerializeStruct, Serializer};

use radicle::cob::ObjectId;

use tuirealm::terminal::TerminalBridge;
use tuirealm::tui::layout::Rect;
use tuirealm::Frame;
use tuirealm::{Application, EventListenerCfg, NoUserEvent};

pub mod cob;
pub mod context;
pub mod log;
pub mod ui;

use context::Context;
use ui::theme::Theme;

/// Trait that must be implemented by client applications in order to be run
/// as tui-application using tui-realm. Implementors act as models to the
/// tui-realm application that can be polled for new messages, updated
/// accordingly and rendered with new state.
///
/// Please see `examples/` for further information on how to use it.
pub trait Tui<Id, Message, Return>
where
    Id: Eq + PartialEq + Clone + Hash,
    Message: Eq,
{
    /// Should initialize an application by mounting and activating components.
    fn init(&mut self, app: &mut Application<Id, Message, NoUserEvent>) -> Result<()>;

    /// Should update the current state by handling a message from the view. Returns true
    /// if view should be updated (e.g. a message was received and the current state changed).
    fn update(&mut self, app: &mut Application<Id, Message, NoUserEvent>) -> Result<bool>;

    /// Should draw the application to a frame.
    fn view(&mut self, app: &mut Application<Id, Message, NoUserEvent>, frame: &mut Frame);

    /// Should return `Some` if the application is requested to quit.
    fn exit(&self) -> Option<Exit<Return>>;
}

/// An optional return value.
pub struct Exit<T> {
    pub value: Option<T>,
}

/// The output that is returned by all selection interfaces.
#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct SelectionExit {
    operation: Option<String>,
    ids: Vec<ObjectId>,
    args: Vec<String>,
}

impl SelectionExit {
    pub fn with_operation(mut self, operation: String) -> Self {
        self.operation = Some(operation);
        self
    }

    pub fn with_id(mut self, id: ObjectId) -> Self {
        self.ids.push(id);
        self
    }

    pub fn with_args(mut self, arg: String) -> Self {
        self.args.push(arg);
        self
    }
}

impl Serialize for SelectionExit {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("", 3)?;
        state.serialize_field("operation", &self.operation)?;
        state.serialize_field(
            "ids",
            &self
                .ids
                .iter()
                .map(|id| format!("{}", id))
                .collect::<Vec<_>>(),
        )?;
        state.serialize_field("args", &self.args)?;
        state.end()
    }
}

/// A tui-window using the cross-platform Terminal helper provided
/// by tui-realm.
pub struct Window {
    /// Helper around `Terminal` to quickly setup and perform on terminal.
    pub terminal: TerminalBridge,
}

impl Default for Window {
    fn default() -> Self {
        Self::new()
    }
}

/// Provides a way to create and run a new tui-application.
impl Window {
    /// Creates a tui-window using the default cross-platform Terminal
    /// helper and panics if its creation fails.
    pub fn new() -> Self {
        let terminal = TerminalBridge::new().expect("Cannot create terminal bridge");

        Self { terminal }
    }

    /// Runs this tui-window with the tui-application given and performs the
    /// following steps:
    /// 1. Enter alternative terminal screen
    /// 2. Run main loop until application should quit and with each iteration
    ///    - poll new events (tick or user event)
    ///    - update application state
    ///    - redraw view
    /// 3. Leave alternative terminal screen
    pub fn run<T, Id, Message, Return>(
        &mut self,
        tui: &mut T,
        interval: u64,
    ) -> Result<Option<Return>>
    where
        T: Tui<Id, Message, Return>,
        Id: Eq + PartialEq + Clone + Hash,
        Message: Eq,
    {
        let mut update = true;
        let mut resize = false;
        let mut size = Rect::default();
        let mut app = Application::init(
            EventListenerCfg::default().default_input_listener(Duration::from_millis(interval)),
        );
        tui.init(&mut app)?;

        while tui.exit().is_none() {
            if update || resize {
                self.terminal
                    .raw_mut()
                    .draw(|frame| tui.view(&mut app, frame))?;
            }
            update = tui.update(&mut app)?;

            resize = size != self.terminal.raw().size()?;
            size = self.terminal.raw().size()?;
        }

        Ok(tui.exit().unwrap().value)
    }
}

/// `tuirealm`'s event and prop system is designed to work with flat component hierarchies.
/// Building deep nested component hierarchies would need a lot more additional effort to
/// properly pass events and props down these hierarchies. This makes it hard to implement
/// full app views (home, patch details etc) as components.
///
/// View pages take into account these flat component hierarchies, and provide
/// switchable sets of components.
pub trait ViewPage<Id, Message>
where
    Id: Eq + PartialEq + Clone + Hash,
    Message: Eq,
{
    /// Will be called whenever a view page is pushed onto the page stack. Should create and mount all widgets.
    fn mount(
        &mut self,
        app: &mut Application<Id, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
    ) -> Result<()>;

    /// Will be called whenever a view page is popped from the page stack. Should unmount all widgets.
    fn unmount(&self, app: &mut Application<Id, Message, NoUserEvent>) -> Result<()>;

    /// Will be called whenever a view page is on top of the stack and can be used to update its internal
    /// state depending on the message passed.
    fn update(
        &mut self,
        app: &mut Application<Id, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
        message: Message,
    ) -> Result<Option<Message>>;

    /// Will be called whenever a view page is on top of the page stack and needs to be rendered.
    fn view(&mut self, app: &mut Application<Id, Message, NoUserEvent>, frame: &mut Frame);

    /// Will be called whenever this view page is pushed to the stack, or it is on top of the stack again
    /// after another view page was popped from the stack.
    fn subscribe(&self, app: &mut Application<Id, Message, NoUserEvent>) -> Result<()>;

    /// Will be called whenever this view page is on top of the stack and another view page is pushed
    /// to the stack, or if this is popped from the stack.
    fn unsubscribe(&self, app: &mut Application<Id, Message, NoUserEvent>) -> Result<()>;
}

/// View pages need to preserve their state (e.g. selected navigation tab, contents
/// and the selected row of a table). Therefor they should not be (re-)created
/// each time they are displayed.
/// Instead the application can push a new page onto the page stack if it needs to
/// be displayed. Its components are then created using the internal state. If a
/// new page needs to be displayed, it will also be pushed onto the stack. Leaving
/// that page again will pop it from the stack. The application can then return to
/// the previously displayed page in the state it was left.
#[derive(Default)]
pub struct PageStack<Id, Message>
where
    Id: Eq + PartialEq + Clone + Hash,
    Message: Eq,
{
    pages: Vec<Box<dyn ViewPage<Id, Message>>>,
}

impl<Id, Message> PageStack<Id, Message>
where
    Id: Eq + PartialEq + Clone + Hash,
    Message: Eq,
{
    pub fn push(
        &mut self,
        mut page: Box<dyn ViewPage<Id, Message>>,
        app: &mut Application<Id, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
    ) -> Result<()> {
        if let Some(page) = self.pages.last() {
            page.unsubscribe(app)?;
        }

        page.mount(app, context, theme)?;
        page.subscribe(app)?;

        self.pages.push(page);

        Ok(())
    }

    pub fn pop(&mut self, app: &mut Application<Id, Message, NoUserEvent>) -> Result<()> {
        self.peek_mut()?.unsubscribe(app)?;
        self.peek_mut()?.unmount(app)?;
        self.pages.pop();

        self.peek_mut()?.subscribe(app)?;

        Ok(())
    }

    pub fn peek_mut(&mut self) -> Result<&mut Box<dyn ViewPage<Id, Message>>> {
        match self.pages.last_mut() {
            Some(page) => Ok(page),
            None => Err(anyhow::anyhow!(
                "Could not peek active page. Page stack is empty."
            )),
        }
    }
}
