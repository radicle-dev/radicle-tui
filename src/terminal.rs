use std::io::{self, Write};
use std::thread;
use std::time::Instant;

use ratatui::termion::screen::{AlternateScreen, IntoAlternateScreen};
use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};

use ratatui::{prelude::*, CompletedFrame};
use ratatui::{TerminalOptions, Viewport};

use tokio::sync::mpsc::{self};

use super::event::Event;

pub type Backend<S> = TermionBackendExt<S>;

pub type InlineTerminal = ratatui::Terminal<Backend<RawTerminal<io::Stdout>>>;
pub type FullscreenTerminal = ratatui::Terminal<Backend<AlternateScreen<RawTerminal<io::Stdout>>>>;

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

/// Spawn one thread that polls `stdin` for new user input and another thread
/// that polls UNIX signals, e.g. `SIGWINCH` when the terminal window size is
/// being changed.
pub fn events() -> mpsc::UnboundedReceiver<Event> {
    let (tx, rx) = mpsc::unbounded_channel();
    let events_tx = tx.clone();
    thread::spawn(move || {
        let start = Instant::now();
        let stdin = io::stdin();
        for key in stdin.keys().flatten() {
            // TODO(erikli): Remove this hack! Perhaps use `tokio::CancellationToken`?
            if start.elapsed().as_millis() > 200 && events_tx.send(Event::Key(key)).is_err() {
                return;
            }
        }
    });

    let events_tx = tx.clone();
    if let Ok(mut signals) = signal_hook::iterator::Signals::new([libc::SIGWINCH]) {
        thread::spawn(move || {
            for _ in signals.forever() {
                if events_tx.send(Event::Resize).is_err() {
                    return;
                }
            }
        });
    }
    rx
}
