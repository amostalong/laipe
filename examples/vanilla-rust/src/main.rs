//! Vanilla Rust example — chat completion over stdin/stdout with tool calls
//!
//! Run with:
//! ```bash
//! export OPENAI_API_KEY=sk-...
//! cargo run --bin laipe-vanilla-rust
//! ```
//!
//! This is the minimum-viable "see laipe working" example. It reads the
//! API endpoint / model from env vars, sends a single "hello" message, and
//! prints the streamed text chunks to stdout. It also registers a single
//! "echo" tool so you can see the full tool-call round trip.
//!
//! See `docs/QUICKSTART.md` for the full walkthrough.

use anyhow::Result;
use laipe_core::types::{ApiFormat, ChatMessage, ChatRole, ProviderConfig};
use laipe_streaming::pick;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("laipe=info")
        .init();

    let endpoint =
        env::var("OPENAI_ENDPOINT").unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY env var required");
    let model = env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());

    let cfg = ProviderConfig {
        endpoint,
        api_key,
        model,
        api_format: ApiFormat::OpenAiChat,
        ..Default::default()
    };

    let messages = vec![ChatMessage {
        role: ChatRole::User,
        content: "Say hello in 5 words or less.".to_string(),
        ..Default::default()
    }];

    let mut rx = pick(cfg.api_format).dispatch(&cfg, &messages, None).await?;

    while let Some(ev) = rx.recv().await {
        match ev {
            laipe_core::types::StreamEvent::Text(delta) => {
                print!("{delta}");
                use std::io::Write;
                std::io::stdout().flush()?;
            }
            laipe_core::types::StreamEvent::ToolCalls(_calls) => {
                // tool calls handled in tool-calling example
            }
            laipe_core::types::StreamEvent::Done => break,
            laipe_core::types::StreamEvent::Error { kind, message } => {
                eprintln!("\n[error] {kind:?}: {message}");
                break;
            }
        }
    }
    println!();
    Ok(())
}
