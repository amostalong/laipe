//! `CancelHandle` — drop to abort an in-flight chat.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Cheap, clone-able abort handle. Drop the handle (or call `cancel()`) to
/// signal the consumer task to stop reading from the stream.
#[derive(Debug, Clone, Default)]
pub struct CancelHandle {
    flag: Arc<AtomicBool>,
}

impl CancelHandle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}
