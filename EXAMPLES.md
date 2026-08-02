# Examples

`laipe` is a library, not a runnable product. The examples in this repo
are **integration tests and onboarding material**, not a CLI suite.

| Example | Status | What it shows | Run it |
|---|---|---|---|
| [`examples/vanilla-rust`](examples/vanilla-rust) | ✅ v0.1.1 — works | Minimum-viable: 1 binary, 1 message, 1 model. Prints streamed text chunks to stdout. | `OPENAI_API_KEY=sk-... cargo run --bin laipe-vanilla-rust` |
| `examples/tauri-minimal` | ❌ v0.2 — planned | Tauri 2 desktop app, 1 chat tab + 1 example tab, end-to-end. | _not yet_ |
| `examples/vanilla-web` | ❌ v0.2 — planned | Pure HTML + JS, no Tauri. Uses `packages/laipe-ts` fetch SSE. | _not yet_ |
| `examples/electron-minimal` | ❌ v0.3 — planned | Electron desktop demo. | _not yet_ |

## `examples/vanilla-rust` — minimum viable

```bash
# from the workspace root
export OPENAI_API_KEY=sk-...
cargo run --bin laipe-vanilla-rust
```

You should see a single chat round-trip:

```
hello there
```

That's the whole thing. The example:

- Reads `OPENAI_ENDPOINT` (defaults to `https://api.openai.com/v1`)
- Reads `OPENAI_API_KEY` (required)
- Reads `OPENAI_MODEL` (defaults to `gpt-4o-mini`)
- Sends one user message: "Say hello in 5 words or less."
- Prints streamed text chunks to stdout as they arrive
- Exits when the stream ends

The full source is `examples/vanilla-rust/src/main.rs` — about 60 lines.
It demonstrates the canonical consumer pattern:

```rust
let mut rx = pick(cfg.api_format).dispatch(&cfg, &messages, None).await?;

while let Some(ev) = rx.recv().await {
    match ev {
        StreamEvent::Text(delta) => print!("{delta}"),
        StreamEvent::ToolCalls(_) => { /* would dispatch tool here */ }
        StreamEvent::Done => break,
        StreamEvent::Error { kind, message } => eprintln!("[error] {kind:?}: {message}"),
    }
}
```

## `examples/tauri-minimal` — what it'll look like (v0.2)

```bash
# after v0.2 lands
cd examples/tauri-minimal
cargo tauri dev
```

You should see a Tauri 2 window open with:
- A left sidebar with 2 tabs: `chat` and `example`
- The `chat` tab is a real working chat with OpenAI: type a message, see streamed response
- The `example` tab is a demo of one `update_doc_item` tool: type some text, click "Apply", and a counter in the UI increments

This example is the on-ramp for new users. The goal is "5 minutes from `git clone` to a working chat in a desktop window". It also serves as a reference for the Tauri 2 glue pattern (`laipe-tokio::run_to_completion` + `tauri::Emitter`).

## Writing your own example

If you build something with `laipe`, open a PR to add it to this directory. A few guidelines:

- **One example per directory**, with its own `Cargo.toml`
- **The example's `Cargo.toml` should depend on the workspace** — `laipe = { path = "../../crates/laipe-streaming" }` etc.
- **The example should have a `README.md`** that says what it shows, how to run it, and what env vars it needs
- **The example should not be in the workspace** if you don't want it to build with `cargo build --workspace`. Add it to the workspace's `members = [...]` only if you want it gated by the same CI checks.

## Why so few examples?

Because v0.1 is the **protocol layer**. The interesting examples (Tauri, Electron, Vue, React) are downstream of the protocol layer being stable. PlotCraft is the first real user; the second will be a Tauri minimal demo; the third will be whoever opens the next PR.

The protocol layer has **25 unit tests** that exercise the build-body, parse-frame, and event-translation logic per protocol. That's the right test surface for v0.1. The examples come in v0.2.
