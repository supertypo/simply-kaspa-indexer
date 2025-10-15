use log::warn;
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(windows)]
use tokio::signal::ctrl_c;
#[cfg(unix)]
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;

#[derive(Clone)]
pub struct SignalHandler {
    shutdown_tx: Sender<()>,
    shutdown_sent: Arc<AtomicBool>,
}

impl SignalHandler {
    pub fn new() -> SignalHandler {
        let (shutdown_tx, _) = broadcast::channel(1);
        SignalHandler { shutdown_tx, shutdown_sent: Arc::new(AtomicBool::new(false)) }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown_sent.load(Ordering::Relaxed)
    }

    pub fn spawn(self) -> Self {
        let h = self.clone();
        tokio::spawn(async move {
            h.run().await;
        });
        self
    }

    async fn run(&self) {
        #[cfg(unix)]
        {
            let mut sigterm = signal(SignalKind::terminate()).expect("Failed to set up SIGTERM handler");
            let mut sigint = signal(SignalKind::interrupt()).expect("Failed to set up SIGINT handler");
            loop {
                tokio::select! {
                    _ = sigint.recv() => {
                        self.handle_signal("SIGINT");
                    },
                    _ = sigterm.recv() => {
                        self.handle_signal("SIGTERM");
                    },
                }
            }
        }
        #[cfg(windows)]
        {
            loop {
                tokio::select! {
                    _ = ctrl_c() => {
                        self.handle_signal("Ctrl+C");
                    },
                }
            }
        }
    }

    fn handle_signal(&self, signal: &str) {
        if self.shutdown_sent.load(Ordering::Relaxed) {
            warn!("{} received, terminating...", signal);
            process::exit(1);
        }
        warn!("{} received, stopping... (repeat for forced close)", signal);
        self.shutdown_sent.store(true, Ordering::Relaxed);
        let _ = self.shutdown_tx.send(());
    }
}

impl Default for SignalHandler {
    fn default() -> Self {
        Self::new()
    }
}
