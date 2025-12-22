use std::io;
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::mpsc::UnboundedSender;

use tokio_util::sync::CancellationToken;

use ratatui::prelude::*;
use ratatui::{CompletedFrame, TerminalOptions, Viewport};

use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};

use super::event::Event;
use super::{Interrupted, Share};

pub type Backend<S> = CrosstermBackend<S>;
pub type InlineTerminal = ratatui::Terminal<Backend<io::Stdout>>;
pub type FullscreenTerminal = ratatui::Terminal<Backend<io::Stdout>>;

const STDIN_TICK_RATE: Duration = Duration::from_millis(20);

pub enum Terminal {
    Inline(InlineTerminal),
    Fullscreen(FullscreenTerminal),
}

impl Terminal {
    pub fn restore(&mut self) -> io::Result<()> {
        match self {
            Terminal::Fullscreen(inner) => {
                disable_raw_mode()?;
                execute!(io::stdout(), LeaveAlternateScreen)?;
                inner.clear()?;
            }
            Terminal::Inline(inner) => {
                disable_raw_mode()?;
                inner.clear()?;
            }
        }
        Ok(())
    }

    pub fn clear(&mut self) -> io::Result<()> {
        match self {
            Terminal::Fullscreen(inner) | Terminal::Inline(inner) => {
                inner.clear()?;
            }
        }
        Ok(())
    }

    pub fn draw<F>(&mut self, f: F) -> io::Result<CompletedFrame<'_>>
    where
        F: FnOnce(&mut Frame),
    {
        match self {
            Terminal::Inline(inner) => inner.draw(f),
            Terminal::Fullscreen(inner) => inner.draw(f),
        }
    }
}

impl TryFrom<Viewport> for Terminal {
    type Error = anyhow::Error;

    fn try_from(viewport: Viewport) -> Result<Self, Self::Error> {
        match viewport {
            Viewport::Fullscreen => {
                execute!(io::stdout(), EnterAlternateScreen)?;
                let options = TerminalOptions { viewport };
                let mut terminal = ratatui::init_with_options(options);

                terminal.clear()?;

                Ok(Terminal::Fullscreen(terminal))
            }
            _ => {
                let options = TerminalOptions { viewport };
                let terminal = ratatui::init_with_options(options);

                Ok(Terminal::Inline(terminal))
            }
        }
    }
}

#[derive(Default)]
pub struct StdinReader {}

impl StdinReader {
    pub async fn run<P: Share>(
        self,
        event_tx: UnboundedSender<Event>,
        mut interrupt_rx: broadcast::Receiver<Interrupted<P>>,
    ) -> anyhow::Result<Interrupted<P>> {
        use ratatui::crossterm::event;

        let token = CancellationToken::new();
        let token_clone = token.clone();

        let key_event_tx = event_tx.clone();
        let key_listener = tokio::spawn(async move {
            loop {
                match event::poll(STDIN_TICK_RATE) {
                    Ok(true) => match event::read() {
                        Ok(event) => {
                            if key_event_tx.send(event.into()).is_err() {
                                return;
                            }
                        }
                        Err(err) => {
                            log::error!(target: "terminal", "Could not read from stdin: {err}");
                        }
                    },
                    _ => {
                        if token_clone.is_cancelled() {
                            break;
                        }
                    }
                }
            }
        });

        let result: anyhow::Result<Interrupted<P>> = tokio::select! {
            Ok(interrupted) = interrupt_rx.recv() => {
                token.cancel();
                Ok(interrupted)
            }
        };
        key_listener.await?;

        result
    }
}
