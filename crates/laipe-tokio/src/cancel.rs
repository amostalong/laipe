//! `CancelHandle` — drop to abort an in-flight chat.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::Notify;

/// Cheap, clone-able abort handle. Drop the handle (or call `cancel()`) to
/// signal the consumer task to stop reading from the stream.
///
/// `cancelled().await` resolves as soon as `cancel()` is called, so
/// callers can `tokio::select!` on it alongside other futures (e.g.
/// oneshot receivers for tool-approval waiters) without polling.
#[derive(Debug, Clone, Default)]
pub struct CancelHandle {
    flag: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl CancelHandle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
        // Wake every task currently parked in `cancelled()`. New
        // callers arriving after this point see the flag and return
        // immediately without parking.
        self.notify.notify_waiters();
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// Resolves when `cancel()` is called. Returns immediately if the
    /// handle is already cancelled. Designed for use inside
    /// `tokio::select!` so a pending tool-approval can be unblocked
    /// the moment the user hits the global Cancel button.
    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        self.notify.notified().await;
    }
}
