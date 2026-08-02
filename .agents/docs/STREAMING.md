# Streaming & anti-stutter

The hard part of an LLM chat UI is **keeping the main thread responsive at
1k tokens/second**. Locus (the editor tool `laipe`'s siblings were built
for) measured 4 specific stutter sources; `laipe` ports their fixes
unchanged.

## The 4 stutter sources & their fixes

| # | Source | Counter in laipe |
|---|---|---|
| 1 | SSE byte → JSON parse blocks the tokio worker pool | `tokio::task::spawn_blocking` isolates the parse loop |
| 2 | 1k tok/s = 1k IPC emits/s floods the consumer | `mpsc::channel(64)` decouples parse from emit; consumer pulls at its own pace |
| 3 | Per-token emit cost dominates rAF budget | 16ms rAF + 256-char batch on the emit side |
| 4 | Downstream `shallowRef`/state invalidated every chunk | Identity-stable: `appendChunk` only mutates `currentText`; never touches `messages[]` reference |

The first two live in `laipe-streaming`. The third lives in the consumer
(TS: `lib/llm-connection.ts`; Rust: `laipe-tokio::run_to_completion`). The
fourth lives entirely in the consumer's state machine — `laipe` doesn't
see it, but `packages/laipe-vue` will ship a `useStreamReducer` that
demonstrates the pattern.

## StreamEvent flow

```
upstream SSE bytes
   ↓
spawn_blocking → SSE parse → JSON value
   ↓
mpsc::channel(64)  ←  throttled at 16ms rAF / 256 chars
   ↓
tokio::sync::mpsc::Receiver<StreamEvent>
   ↓
your state machine → UI
```

Every chunk is exactly one `StreamEvent`:

```rust
pub enum StreamEvent {
    Text(String),                     // append to `currentText`
    ToolCalls(Vec<ToolCallPartial>),  // accumulate by `index`; emit full ToolCallInfo on Done
    Done,                             // stream finished cleanly
}
```

## Cancel

Drop a `CancelHandle` (or call `.cancel()`) and the in-flight chat stops
reading from the receiver. The upstream TCP socket is closed at the next
SSE boundary. The consumer sees a `Drop` of its `Receiver`; the parse task
sees its `Sender` close and unwinds.

`laipe` does **not** implement Anthropic-style mid-stream cancel
interrupts (where you can send a partial response and ask the model to
extend). That's a v0.2+ feature.
