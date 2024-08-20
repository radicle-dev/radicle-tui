use std::fmt::Debug;
use std::time::Duration;

use ratatui::Frame;
use termion::event::Key;
use tokio::sync::broadcast;
use tokio::sync::mpsc::UnboundedReceiver;

use anyhow::Result;

use crate::event::Event;
use crate::store;
use crate::store::State;
use crate::task;
use crate::task::Interrupted;
use crate::terminal;
use crate::Channel;

const RENDERING_TICK_RATE: Duration = Duration::from_millis(250);
const INLINE_HEIGHT: usize = 20;

pub trait App {
    type State;
    type Message;

    fn render(&self, frame: &mut Frame, ui: UI, state: &Self::State) -> Result<()>;
}

pub async fn run_app<S, M, P>(
    channel: Channel<M>,
    state: S,
    app: impl App<State = S, Message = M>,
) -> Result<Option<P>>
where
    S: State<P, Message = M> + Clone + Debug + Send + Sync + 'static,
    M: 'static,
    P: Clone + Debug + Send + Sync + 'static,
{
    let (terminator, mut interrupt_rx) = task::create_termination();

    let (store, state_rx) = store::Store::<S, M, P>::new();
    let frontend = Frontend::default();

    tokio::try_join!(
        store.main_loop(state, terminator, channel.rx, interrupt_rx.resubscribe()),
        frontend.im_main_loop(app, state_rx, interrupt_rx.resubscribe()),
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

#[derive(Default)]
pub struct Frontend {}

impl Frontend {
    pub async fn im_main_loop<S, M, P>(
        self,
        app: impl App<State = S, Message = M>,
        mut state_rx: UnboundedReceiver<S>,
        mut interrupt_rx: broadcast::Receiver<Interrupted<P>>,
    ) -> anyhow::Result<Interrupted<P>>
    where
        S: State<P> + 'static,
        M: 'static,
        P: Clone + Send + Sync + Debug,
    {
        let mut ticker = tokio::time::interval(RENDERING_TICK_RATE);

        let mut terminal = terminal::setup(INLINE_HEIGHT)?;
        let mut events_rx = terminal::events();

        let mut state = state_rx.recv().await.unwrap();

        let mut ui = UI::default();
        terminal.draw(|frame| {
            if let Err(err) = app.render(frame, ui.clone(), &state) {
                log::warn!("Drawing failed: {}", err);
            }
        })?;

        let result: anyhow::Result<Interrupted<P>> = loop {
            tokio::select! {
                // Tick to terminate the select every N milliseconds
                _ = ticker.tick() => (),
                Some(event) = events_rx.recv() => match event {
                    Event::Key(key) => ui.handle_event(key),
                    Event::Resize => (),
                },
                // Handle state updates
                Some(s) = state_rx.recv() => {
                    state = s;
                },
                // Catch and handle interrupt signal to gracefully shutdown
                Ok(interrupted) = interrupt_rx.recv() => {
                    let size = terminal.get_frame().size();
                    let _ = terminal.set_cursor(size.x, size.y);

                    break Ok(interrupted);
                }
            }
            terminal.draw(|frame| {
                if let Err(err) = app.render(frame, ui.clone(), &state) {
                    log::warn!("Drawing failed: {}", err);
                }
            })?;
        };

        terminal::restore(&mut terminal)?;

        result
    }
}

pub struct Response {}

pub trait Widget {
    fn ui(self, ui: &mut UI) -> Response;
}

#[derive(Default, Clone)]
pub struct UI {
    events: Vec<Event>,
}

impl UI {
    pub fn input(&mut self, f: impl Fn(Key) -> bool) -> bool {
        self.events
            .iter()
            .find(|ev| match ev {
                Event::Key(key) => f(*key),
                _ => false,
            })
            .is_some()
    }
}

impl UI {
    pub fn handle_event(&mut self, key: Key) {
        self.events.push(Event::Key(key));
    }
}

impl UI {
    pub fn shortcuts(&mut self) -> Response {
        widget::Shortcuts::new().ui(self)
    }

    pub fn textview(&mut self, _text: String) -> Response {
        widget::TextView::new().ui(self)
    }
}

mod widget {
    use super::{Response, Widget, UI};

    pub struct TextView {}

    impl TextView {
        pub fn new() -> Self {
            Self {}
        }
    }

    impl Widget for TextView {
        fn ui(self, _ui: &mut UI) -> Response {
            // Actually render
            Response {}
        }
    }

    pub struct Shortcuts {}

    impl Shortcuts {
        pub fn new() -> Self {
            Self {}
        }
    }

    impl Widget for Shortcuts {
        fn ui(self, _ui: &mut UI) -> Response {
            // Actually render
            Response {}
        }
    }
}
