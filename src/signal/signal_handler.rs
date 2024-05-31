use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use log::warn;
#[cfg(windows)]
use tokio::signal::ctrl_c;
#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};

pub async fn notify_on_signals(run: Arc<AtomicBool>) {
    #[cfg(unix)]
    {
        let mut sigterm = signal(SignalKind::terminate()).expect("Failed to set up SIGTERM handler");
        let mut sigint = signal(SignalKind::interrupt()).expect("Failed to set up SIGINT handler");
        loop {
            tokio::select! {
                _ = sigint.recv() => {
                    exit(run.clone(), "SIGINT");
                },
                _ = sigterm.recv() => {
                    exit(run.clone(), "SIGTERM");
                },
            }
        }
    }
    #[cfg(windows)]
    {
        let ctrl_c = ctrl_c();
        loop {
            tokio::select! {
                _ = ctrl_c => {
                    exit(run, "Ctrl+C");
                },
            }
        }
    }
}

fn exit(run: Arc<AtomicBool>, signal: &str) {
    if !run.load(Ordering::Relaxed) {
        warn!("{} received, terminating...", signal);
        process::exit(1);
    }
    warn!("{} received, stopping... (repeat for forced close)", signal);
    run.store(false, Ordering::Relaxed);
}
