//! laipe-tokio — convenience helpers for the tokio runtime
//!
//! Provides:
//! - `CancelHandle` — drop to abort an in-flight chat
//! - `bounded_run!` macro — quick "spin up a chat and forward into a Sender"
//!
//! This crate is intentionally tiny. Most users only need `laipe-streaming`
//! directly; `laipe-tokio` exists so example/demo code can stay short.

#![doc(html_root_url = "https://docs.rs/laipe-tokio/0.1.0")]

pub mod cancel;
pub mod run;

pub use cancel::CancelHandle;
pub use run::run_to_completion;
