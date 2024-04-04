use std::io::{self, Write};
use std::thread;

use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};

use ratatui::prelude::*;

use tokio::sync::mpsc::{self};

use super::event::Event;

type Backend = TermionBackendExt<RawTerminal<io::Stdout>>;

/// FIXME Remove workaround after a new `ratatui` version with
/// https://github.com/ratatui-org/ratatui/pull/981/ included was released.
pub struct TermionBackendExt<W>
where
    W: Write,
{
    cursor: Option<(u16, u16)>,
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

    fn get_cursor(&mut self) -> io::Result<(u16, u16)> {
        match self.inner.get_cursor() {
            Ok((x, y)) => {
                let cursor = (x.saturating_sub(0), y.saturating_sub(0));
                self.cursor = Some(cursor);
                Ok(cursor)
            }
            Err(_) => Ok(self.cursor.unwrap_or((0, 0))),
        }
    }

    fn set_cursor(&mut self, x: u16, y: u16) -> io::Result<()> {
        self.cursor = Some((x, y));
        self.inner.set_cursor(x, y)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: backend::ClearType) -> io::Result<()> {
        self.inner.clear_region(clear_type)
    }

    fn size(&self) -> io::Result<Rect> {
        self.inner.size()
    }

    fn window_size(&mut self) -> io::Result<backend::WindowSize> {
        self.inner.window_size()
    }

    fn flush(&mut self) -> io::Result<()> {
        ratatui::backend::Backend::flush(&mut self.inner)
    }
}

pub fn setup(height: usize) -> anyhow::Result<Terminal<Backend>> {
    let stdout = io::stdout().into_raw_mode()?;
    let options = TerminalOptions {
        viewport: Viewport::Inline(height as u16),
    };

    Ok(Terminal::with_options(
        TermionBackendExt::new(stdout),
        options,
    )?)
}

pub fn restore(terminal: &mut Terminal<Backend>) -> anyhow::Result<()> {
    terminal.clear()?;
    Ok(())
}

pub fn events() -> mpsc::UnboundedReceiver<Event> {
    let (tx, rx) = mpsc::unbounded_channel();
    let events_tx = tx.clone();
    thread::spawn(move || {
        let stdin = io::stdin();
        for key in stdin.keys().flatten() {
            if events_tx.send(Event::Key(key)).is_err() {
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
