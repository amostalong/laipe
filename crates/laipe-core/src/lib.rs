//! laipe-core — protocol-agnostic types and error definitions
//!
//! This crate contains the shared vocabulary used by all laipe streaming
//! implementations: chat message shapes, tool definitions, error kinds, and
//! diagnostic info. It has no HTTP/streaming dependencies — pure types only.
//!
//! See `docs/PROTOCOLS.md` for how these types map to each LLM provider.

#![doc(html_root_url = "https://docs.rs/laipe-core/0.1.0")]

pub mod diagnostics;
pub mod error;
pub mod tool;
pub mod types;

pub use diagnostics::ErrorContext;
pub use error::{ChatErrorDiag, ChatErrorKind};
pub use tool::{ToolCallInfo, ToolCallPartial, ToolDefinition, ToolResult};
pub use types::{
    ApiFormat, ChatMessage, ChatRole, ChatStatus, EffortLevel, ProviderConfig, StreamEvent,
    TestProviderParams, TestProviderResult,
};
