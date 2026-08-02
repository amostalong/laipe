//! Emit-side throttling helpers.
//!
//! The 16ms rAF + 256-char batch trick keeps main-thread / IPC pressure
//! flat even at 1k tok/s. The two knobs live here so all three protocol
//! implementations can share them.

/// Default minimum interval between emit batches (rounded to a frame).
pub const DEFAULT_BATCH_INTERVAL_MS: u64 = 16;

/// Default maximum characters per emit batch.
pub const DEFAULT_BATCH_MAX_CHARS: usize = 256;
