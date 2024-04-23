use std::fmt::Debug;

#[cfg(unix)]
use tokio::signal::unix::signal;
use tokio::sync::broadcast;

/// An `Interrupt` message that is produced by either an OS signal (e.g. kill)
/// or the user by requesting the application to close.
#[derive(Debug, Clone)]
pub enum Interrupted<P>
where
    P: Clone + Send + Sync + Debug,
{
    OsSignal,
    User { payload: Option<P> },
}

/// The `Terminator` wraps a broadcast channel and can send an interrupt messages.
#[derive(Debug, Clone)]
pub struct Terminator<P>
where
    P: Clone + Send + Sync + Debug,
{
    interrupt_tx: broadcast::Sender<Interrupted<P>>,
}

impl<P> Terminator<P>
where
    P: Clone + Send + Sync + Debug + 'static,
{
    /// Create a `Terminator` that stores the sending end of a broadcast channel.
    pub fn new(interrupt_tx: broadcast::Sender<Interrupted<P>>) -> Self {
        Self { interrupt_tx }
    }

    /// Send interrupt message to the broadcast channel.
    pub fn terminate(&mut self, interrupted: Interrupted<P>) -> anyhow::Result<()> {
        self.interrupt_tx.send(interrupted)?;

        Ok(())
    }
}

/// Receive `SIGINT` and call terminator which sends the interrupt message to its broadcast channel.
#[cfg(unix)]
async fn terminate_by_unix_signal<P>(mut terminator: Terminator<P>)
where
    P: Clone + Send + Sync + Debug + 'static,
{
    let mut interrupt_signal = signal(tokio::signal::unix::SignalKind::interrupt())
        .expect("failed to create interrupt signal stream");

    interrupt_signal.recv().await;

    terminator
        .terminate(Interrupted::OsSignal)
        .expect("failed to send interrupt signal");
}

/// Create a broadcast channel and spawn a task for retrieving the applications' kill signal.
pub fn create_termination<P>() -> (Terminator<P>, broadcast::Receiver<Interrupted<P>>)
where
    P: Clone + Send + Sync + Debug + 'static,
{
    let (tx, rx) = broadcast::channel(1);
    let terminator = Terminator::new(tx);

    #[cfg(unix)]
    tokio::spawn(terminate_by_unix_signal(terminator.clone()));

    (terminator, rx)
}
