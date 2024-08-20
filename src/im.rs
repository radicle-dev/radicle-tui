use std::collections::VecDeque;
use std::fmt::Debug;
use std::time::Duration;

use anyhow::Result;

use ratatui::layout::{Layout, Rect};
use tokio::sync::broadcast;
use tokio::sync::mpsc::UnboundedReceiver;

use termion::event::Key;

use ratatui::Frame;

use crate::event::Event;
use crate::store;
use crate::store::State;
use crate::task;
use crate::task::Interrupted;
use crate::terminal;
use crate::ui::theme::Theme;
use crate::Channel;

const RENDERING_TICK_RATE: Duration = Duration::from_millis(250);
const INLINE_HEIGHT: usize = 20;

pub trait App {
    type State;
    type Message;

    fn render(&self, ui: &mut UI, frame: &mut Frame, state: &Self::State) -> Result<()>;
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

        let result: anyhow::Result<Interrupted<P>> = loop {
            tokio::select! {
                // Tick to terminate the select every N milliseconds
                _ = ticker.tick() => (),
                Some(event) = events_rx.recv() => match event {
                    Event::Key(key) => ui.store_input(key),
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
                let mut ui = ui.clone().with_area(frame.size());
                if let Err(err) = app.render(&mut ui, frame, &state) {
                    log::warn!("Drawing failed: {}", err);
                }
            })?;

            ui.clear_inputs();
        };

        terminal::restore(&mut terminal)?;

        result
    }
}

#[derive(Debug)]
pub struct Response {}

#[derive(Debug)]
pub struct InnerResponse<R> {
    /// What the user closure returned.
    pub inner: R,
    /// The response of the area.
    pub response: Response,
}

impl<R> InnerResponse<R> {
    #[inline]
    pub fn new(inner: R, response: Response) -> Self {
        Self { inner, response }
    }
}

pub trait Widget {
    fn ui(self, ui: &mut UI, frame: &mut Frame) -> Response;
}

#[derive(Default, Clone, Debug)]
pub struct UI {
    pub(crate) inputs: VecDeque<Key>,
    pub(crate) theme: Theme,
    pub(crate) area: Rect,
    pub(crate) layout: Layout,
    next_area: usize,
}

impl UI {
    pub fn input(&mut self, f: impl Fn(Key) -> bool) -> bool {
        self.inputs.iter().find(|key| f(**key)).is_some()
    }

    pub fn store_input(&mut self, key: Key) {
        self.inputs.push_back(key);
    }

    pub fn clear_inputs(&mut self) {
        self.inputs.clear();
    }
}

impl UI {
    pub fn new(area: Rect) -> Self {
        Self {
            area,
            ..Default::default()
        }
    }

    pub fn with_area(mut self, area: Rect) -> Self {
        self.area = area;
        self
    }

    pub fn with_layout(mut self, layout: Layout) -> Self {
        self.layout = layout;
        self
    }

    pub fn with_inputs(mut self, inputs: VecDeque<Key>) -> Self {
        self.inputs = inputs;
        self
    }

    pub fn area(&self) -> Rect {
        self.area
    }

    pub fn next_area(&mut self) -> Option<Rect> {
        let rect = self.layout.split(self.area).get(self.next_area).cloned();
        self.next_area = self.next_area + 1;
        rect
    }
}

impl UI {
    pub fn add(&mut self, frame: &mut Frame, widget: impl Widget) -> Response {
        widget.ui(self, frame)
    }

    pub fn child_ui(&mut self, area: Rect, layout: Layout) -> Self {
        UI::default()
            .with_area(area)
            .with_layout(layout)
            .with_inputs(self.inputs.clone())
    }

    pub fn build_layout<R>(
        &mut self,
        layout: Layout,
        add_contents: impl FnOnce(&mut Self) -> R,
    ) -> InnerResponse<R> {
        self.build_layout_dyn(layout, Box::new(add_contents))
    }

    pub fn build_layout_dyn<'a, R>(
        &mut self,
        layout: Layout,
        add_contents: Box<dyn FnOnce(&mut Self) -> R + 'a>,
    ) -> InnerResponse<R> {
        let mut child_ui = self.child_ui(self.area(), layout);
        let inner = add_contents(&mut child_ui);

        InnerResponse::new(inner, Response {})
    }
}

impl UI {
    pub fn shortcuts(
        &mut self,
        frame: &mut Frame,
        shortcuts: &[(String, String)],
        divider: char,
    ) -> Response {
        widget::Shortcuts::new(shortcuts, divider).ui(self, frame)
    }

    pub fn textview(&mut self, frame: &mut Frame, text: String) -> Response {
        widget::TextView::new(text).ui(self, frame)
    }
}

mod widget {
    use ratatui::style::Stylize;
    use ratatui::text::Text;
    use ratatui::widgets::Row;
    use ratatui::Frame;
    use ratatui::{layout::Constraint, widgets::Paragraph};

    use crate::ui::theme::style;

    use super::{Response, Widget, UI};

    pub struct TextView {
        text: String,
    }

    impl TextView {
        pub fn new(text: impl ToString) -> Self {
            Self {
                text: text.to_string(),
            }
        }
    }

    impl Widget for TextView {
        fn ui(self, ui: &mut UI, frame: &mut Frame) -> Response {
            let area = ui.next_area().unwrap_or_default();

            frame.render_widget(Paragraph::new(self.text), area);

            Response {}
        }
    }

    pub struct Shortcuts {
        pub shortcuts: Vec<(String, String)>,
        pub divider: char,
    }

    impl Shortcuts {
        pub fn new(shortcuts: &[(String, String)], divider: char) -> Self {
            Self {
                shortcuts: shortcuts.to_vec(),
                divider,
            }
        }
    }

    impl Widget for Shortcuts {
        fn ui(self, ui: &mut UI, frame: &mut Frame) -> Response {
            use ratatui::widgets::Table;

            let mut shortcuts = self.shortcuts.iter().peekable();
            let mut row = vec![];

            while let Some(shortcut) = shortcuts.next() {
                let short = Text::from(shortcut.0.clone()).style(ui.theme.shortcuts_keys_style);
                let long = Text::from(shortcut.1.clone()).style(ui.theme.shortcuts_action_style);
                let spacer = Text::from(String::new());
                let divider = Text::from(format!(" {} ", self.divider)).style(style::gray().dim());

                row.push((shortcut.0.chars().count(), short));
                row.push((1, spacer));
                row.push((shortcut.1.chars().count(), long));

                if shortcuts.peek().is_some() {
                    row.push((3, divider));
                }
            }

            let row_copy = row.clone();
            let row: Vec<Text<'_>> = row_copy
                .clone()
                .iter()
                .map(|(_, text)| text.clone())
                .collect();
            let widths: Vec<Constraint> = row_copy
                .clone()
                .iter()
                .map(|(width, _)| Constraint::Length(*width as u16))
                .collect();
            let table = Table::new([Row::new(row)], widths).column_spacing(0);

            let area = ui.next_area().unwrap_or_default();
            frame.render_widget(table, area);

            Response {}
        }
    }
}
