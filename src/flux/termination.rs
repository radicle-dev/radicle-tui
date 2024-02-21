use std::fmt::Debug;

#[cfg(unix)]
use tokio::signal::unix::signal;
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub enum Interrupted<P>
where
    P: Clone + Send + Sync + Debug,
{
    OsSignal,
    User { payload: Option<P> },
}

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
    pub fn new(interrupt_tx: broadcast::Sender<Interrupted<P>>) -> Self {
        Self { interrupt_tx }
    }

    pub fn terminate(&mut self, interrupted: Interrupted<P>) -> anyhow::Result<()> {
        self.interrupt_tx.send(interrupted)?;

        Ok(())
    }
}

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

// create a broadcast channel for retrieving the application kill signal
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
