use std::io::{self, Write};
use std::thread;

use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};

use ratatui::prelude::*;

use tokio::sync::mpsc::{self};

use super::event::Event;

type Backend = TermionBackendExt<RawTerminal<io::Stdout>>;

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
