use std::io::{self, IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio_util::sync::CancellationToken;

/// Small terminal renderer. It is deliberately optional for headless hosts.
pub async fn run(player_count: Arc<AtomicUsize>, cancel: CancellationToken) {
    if !io::stdout().is_terminal() {
        tracing::info!("terminal rendering requested, but stdout is not a terminal");
        return;
    }

    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = interval.tick() => {
                print!("\rPomme Server | players: {} | Ctrl+C to stop", player_count.load(Ordering::Relaxed));
                let _ = io::stdout().flush();
            }
        }
    }
    println!();
}
