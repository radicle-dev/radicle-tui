use std::marker::PhantomData;
use std::{fmt::Debug, future::Future};

use tokio::sync::{broadcast, mpsc::UnboundedSender};

use super::{Interrupted, Share};

pub type EmptyProcessors = Vec<EmptyProcessor>;

/// A task that can be run.
pub trait Task: Debug + Send + Sync + 'static {
    type Return;

    fn run(&self) -> anyhow::Result<Vec<Self::Return>>;
}

/// A processor that can be added to the application environment.
/// Processors will receive application messages and can produce new ones.
pub trait Process<M: Share> {
    fn process(&mut self, _message: M) -> impl Future<Output = anyhow::Result<Vec<M>>> + Send;
}

/// An empty processor that does nothing.
#[derive(Debug, Clone)]
pub struct EmptyProcessor;

impl<M: Share> Process<M> for EmptyProcessor {
    async fn process(&mut self, _message: M) -> anyhow::Result<Vec<M>> {
        Ok(vec![])
    }
}

/// A worker that is spawned by the application. Invokes
/// all processors and sends received application messages.
pub struct Worker<P, M, R> {
    work_tx: UnboundedSender<M>,
    _phantom: PhantomData<(P, M, R)>,
}

impl<P, M, R> Worker<P, M, R>
where
    P: Process<M> + Share,
    M: Share,
    R: Share,
{
    pub fn new(tx: UnboundedSender<M>) -> Self {
        Self {
            work_tx: tx,
            _phantom: PhantomData,
        }
    }

    pub async fn run(
        &self,
        processors: Vec<P>,
        mut message_rx: broadcast::Receiver<M>,
        mut interrupt_rx: broadcast::Receiver<Interrupted<R>>,
    ) -> anyhow::Result<Interrupted<R>> {
        let result = loop {
            tokio::select! {
                Ok(message) = message_rx.recv() => {
                    for mut p in processors.clone() {
                        for m in p.process(message.clone()).await? {
                            if let Err(err) = self.work_tx.send(m) {
                                log::error!(target: "worker", "Unable to send message: {err}")
                            }
                        }
                    }
                },
                // Catch and handle interrupt signal to gracefully shutdown
                Ok(interrupted) = interrupt_rx.recv() => {
                    break interrupted;
                }
            }
        };

        Ok(result)
    }
}
