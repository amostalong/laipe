//! `run_to_completion` — forward `Receiver<StreamEvent>` into a `Sender`.
//!
//! Mostly sugar for examples; production code usually wants the raw
//! receiver to feed its own state machine.

use anyhow::Result;
use laipe_core::types::StreamEvent;
use laipe_streaming::StreamError;

/// Run the stream to completion, forwarding every event into `out`.
/// Returns when the upstream sends `Done` or yields a non-`Other` error.
pub async fn run_to_completion(
    rx: &mut tokio::sync::mpsc::Receiver<StreamEvent>,
    out: &tokio::sync::mpsc::Sender<StreamEvent>,
) -> Result<(), StreamError> {
    while let Some(ev) = rx.recv().await {
        if out.send(ev.clone()).await.is_err() {
            break;
        }
        if matches!(ev, StreamEvent::Done) {
            break;
        }
    }
    Ok(())
}
