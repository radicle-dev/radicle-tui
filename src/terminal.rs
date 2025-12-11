use std::io;
use std::io::Write;
use std::thread;
use std::time::Duration;

use signal_hook::iterator::Signals;

use tokio::sync::broadcast;
use tokio::sync::mpsc::UnboundedSender;

use tokio_util::sync::CancellationToken;

use termion::async_stdin;
use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};

use ratatui::prelude::*;
use ratatui::termion::screen::{AlternateScreen, IntoAlternateScreen};
use ratatui::{CompletedFrame, TerminalOptions, Viewport};

use crate::Share;

use super::event::Event;
use super::Interrupted;

pub type Backend<S> = TermionBackendExt<S>;
pub type InlineTerminal = ratatui::Terminal<Backend<RawTerminal<io::Stdout>>>;
pub type FullscreenTerminal = ratatui::Terminal<Backend<AlternateScreen<RawTerminal<io::Stdout>>>>;

const STDIN_TICK_RATE: Duration = Duration::from_millis(20);

pub enum Terminal {
    Inline(InlineTerminal),
    Fullscreen(FullscreenTerminal),
}

impl Terminal {
    pub fn restore(&mut self) -> io::Result<()> {
        match self {
            Terminal::Fullscreen(inner) => {
                inner.clear()?;
            }
            Terminal::Inline(inner) => {
                // TODO(erikli): Check if still needed.
                let area = inner.get_frame().area();
                let position = Position::new(area.x, area.y);
                inner.set_cursor_position(position)?;

                inner.clear()?;
            }
        }

        Ok(())
    }

    pub fn draw<F>(&mut self, f: F) -> io::Result<CompletedFrame>
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
                let stdout = io::stdout().into_raw_mode()?.into_alternate_screen()?;
                let options = TerminalOptions { viewport };
                let mut terminal =
                    ratatui::Terminal::with_options(TermionBackendExt::new(stdout), options)?;

                terminal.clear()?;

                Ok(Terminal::Fullscreen(terminal))
            }
            _ => {
                let stdout = io::stdout().into_raw_mode()?;
                let options = TerminalOptions { viewport };
                let terminal =
                    ratatui::Terminal::with_options(TermionBackendExt::new(stdout), options)?;

                Ok(Terminal::Inline(terminal))
            }
        }
    }
}

/// FIXME Remove workaround after a new `ratatui` version with
/// <https://github.com/ratatui-org/ratatui/pull/981/> included was released.
pub struct TermionBackendExt<W>
where
    W: Write,
{
    cursor: Option<Position>,
    inner: TermionBackend<W>,
}

impl<W> TermionBackendExt<W>
where
    W: Write,
{
    pub fn new(writer: W) -> Self {
        Self {
            cursor: None,
            inner: TermionBackend::new(writer),
        }
    }
}

impl<W: Write> ratatui::backend::Backend for TermionBackendExt<W> {
    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a buffer::Cell)>,
    {
        self.inner.draw(content)
    }

    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        self.inner.append_lines(n)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        match self.inner.get_cursor_position() {
            Ok(position) => {
                self.cursor = Some(position);
                Ok(position)
            }
            Err(_) => Ok(self.cursor.unwrap_or_default()),
        }
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.cursor = Some(position.into());
        self.inner
            .set_cursor_position(self.cursor.unwrap_or_default())
    }

    fn clear(&mut self) -> io::Result<()> {
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: backend::ClearType) -> io::Result<()> {
        self.inner.clear_region(clear_type)
    }

    fn size(&self) -> io::Result<Size> {
        self.inner.size()
    }

    fn window_size(&mut self) -> io::Result<backend::WindowSize> {
        self.inner.window_size()
    }

    fn flush(&mut self) -> io::Result<()> {
        ratatui::backend::Backend::flush(&mut self.inner)
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
        let token = CancellationToken::new();
        let token_clone = token.clone();

        let key_event_tx = event_tx.clone();
        let key_listener = tokio::spawn(async move {
            let mut stdin = async_stdin().keys();
            loop {
                if let Some(Ok(key)) = stdin.next() {
                    if key_event_tx.send(Event::Key(key)).is_err() {
                        return;
                    }
                }
                tokio::select! {
                    _ = token_clone.cancelled() => {
                        break;
                    }
                    _ = tokio::time::sleep(STDIN_TICK_RATE) => {}
                }
            }
        });

        let mut signals = Signals::new([libc::SIGWINCH])?;
        let signal_handle = signals.handle();
        let signal_event_tx = event_tx.clone();
        thread::spawn(move || {
            for _ in signals.forever() {
                if signal_event_tx.send(Event::Resize).is_err() {
                    return;
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
        signal_handle.close();

        result
    }
}
