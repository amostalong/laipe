# laipe-app

A **native desktop chat app** — the laipe starter. Built on Tauri 2 + Vue 3 + Rust + laipe. No browser, no CORS, no leaked API keys in `localStorage`. Single `.exe` per platform. Same code compiles to Windows, macOS, Linux, and (with one config flip) iOS and Android.

This is the **v0.2 deliverable** and the primary deliverable of the laipe project. Fork it, customize it, ship it. For a guided tour of how to extend it into a real agent, see [`EXTENDING.md`](../EXTENDING.md).

## What you get

- **Native window** powered by Tauri 2 (OS webview: WebView2 / WKWebView / WebKitGTK)
- **Multi-conversation** sidebar with `localStorage` persistence
- **Settings modal** for endpoint / API key / model / format
- **Streaming responses** driven by the Rust backend (no CORS)
- **Tool calling** with a small Rust agent loop (2 sample tools: `get_current_time`, `echo`)
- **Cancellation** via a Tauri `cancel` command + `laipe_tokio::CancelHandle`
- **Mobile-ready config** — change one line in `tauri.conf.json` to ship
  iOS / Android builds
- ~10 MB binary per platform, ~50 MB RAM

## Architecture

```
┌──────────────── Tauri window (OS webview) ──────────────────┐
│                                                            │
│   Vue 3 UI (Vite-bundled)                                  │
│        │                                                   │
│        │ invoke('chat', { cfg, messages, tools })          │
│        │ listen('chat:chunk', 'chat:tool_calls', ...)      │
│        ▼                                                   │
│   Tauri IPC                                                │
│        │                                                   │
│        ▼                                                   │
│   Rust backend (src-tauri/src/lib.rs)                      │
│        │                                                   │
│        │  for turn in 0..MAX_AGENT_TURNS:                  │
│        │    rx = pick(format).dispatch(cfg, msgs, tools)   │
│        │    for ev in rx: emit chat:chunk/chat:tool_calls  │
│        │    if tool_calls:                                 │
│        │      execute_tool(name, args) for each call       │
│        │      append assistant + tool messages → loop      │
│        │    else: emit chat:done → return                  │
│        ▼                                                   │
│   mpsc::Receiver<StreamEvent>                              │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

The Vue frontend never calls the LLM API directly. It invokes a Rust
command, which runs the agent loop, which calls laipe, which streams
events back as Tauri events. This means:

- **No CORS**: outbound HTTP is done by Rust, which has no origin restrictions.
- **API key is safe**: stored in the Rust process's config; not in
  `localStorage` (where any browser devtools could read it).
- **Tools are safe too**: any function the LLM can call lives in Rust,
  so it can touch the filesystem, network, or native APIs without
  exposing that surface to the webview.
- **The same code compiles to mobile**: Tauri 2 is the only desktop framework
  that ships iOS + Android + desktop from a single Rust + Vue codebase.

## 30-second setup

```bash
# from the repo root (this dir is one of the workspace members)
bun install
cargo install tauri-cli --version "^2.0"   # one-time, ~5-10 min compile
bun run tauri:dev                          # opens a native window with HMR
```

Click **Settings** (top right), paste your API key, and you're chatting.
Try "what time is it?" — the LLM should call `get_current_time` and
return the current UTC time. The tool call shows up as a card inside the
assistant's message.

## 1-command build

```bash
# from the repo root
cargo tauri build -p laipe-app
# → laipe-app/src-tauri/target/release/bundle/msi/laipe_0.1.0_x64_en-US.msi
# → laipe-app/src-tauri/target/release/laipe.exe                   (portable)

# macOS (must run on a Mac)
cargo tauri build -p laipe-app
# → laipe-app/src-tauri/target/release/bundle/macos/laipe.app
# → laipe-app/src-tauri/target/release/bundle/dmg/laipe_0.1.0_aarch64.dmg
```

## All 5 platforms

Tauri 2 supports iOS and Android as first-class targets. The frontend
(Vue + Vite) and backend (Rust + laipe) compile unchanged. You only need
the platform-specific toolchain installed (Xcode for iOS, Android SDK/NDK
for Android).

| Platform | Build command | Notes |
|---|---|---|
| Windows | `cargo tauri build --target x86_64-pc-windows-msvc` | Default. |
| macOS (Intel) | `cargo tauri build --target x86_64-apple-darwin` | Must run on macOS. |
| macOS (Apple Silicon) | `cargo tauri build --target aarch64-apple-darwin` | Must run on macOS. |
| Linux | `cargo tauri build --target x86_64-unknown-linux-gnu` | |
| **iOS** | `cargo tauri build --target aarch64-apple-ios` | **Mac + Xcode only.** |
| **Android** | `cargo tauri build --target aarch64-linux-android` | Android SDK + NDK required. |

After `tauri build`, the bundlers live under
`src-tauri/target/release/bundle/`. For mobile builds, install the matching
`--target` Rust target with `rustup target add <triple>` first.

## File map

```
laipe-app/
├── package.json                        # frontend deps (Vue, @tauri-apps/api, laipe-ts, laipe-vue)
├── tsconfig.json / tsconfig.node.json
├── vite.config.ts                      # dev server on :5175
├── index.html
├── README.md / .gitignore
│
├── src/                                # Vue 3 frontend
│   ├── main.ts
│   ├── App.vue                         # shell — composes laipe-vue directly
│   ├── tools.ts                        # ToolDefinition[] (get_current_time, echo)
│   ├── style.css                       # CSS variables + resets
│   ├── components/                     # (empty — uses laipe-vue)
│   └── composables/                    # (empty — uses laipe-vue)
│
└── src-tauri/                          # Rust backend
    ├── Cargo.toml                      # chrono dep for get_current_time
    ├── build.rs
    ├── tauri.conf.json
    ├── capabilities/
    │   └── default.json                # IPC permissions (no fs / network / shell by default)
    ├── icons/                          # placeholder PNG icons
    └── src/
        ├── main.rs                     # entry — calls lib::run()
        └── lib.rs                      # chat + cancel commands, AppState, agent loop, execute_tool
```

## Where the laipe magic happens

`src-tauri/src/lib.rs` is the only file that touches the network. The
agent loop is ~50 lines and lives in the `#[tauri::command] async fn chat(...)` body:

```rust
for turn in 0..MAX_AGENT_TURNS {
    let mut rx = laipe_streaming::pick(cfg.api_format)
        .dispatch(&cfg, &working, tools.as_deref())
        .await?;

    let mut tool_calls: Vec<ToolCallPartial> = Vec::new();
    while let Some(ev) = rx.recv().await {
        match ev {
            StreamEvent::Text(d)       => app.emit("chat:chunk", d)?,
            StreamEvent::ToolCalls(p)  => { app.emit("chat:tool_calls", &p)?; tool_calls.extend(p); }
            StreamEvent::Done          => break,
            StreamEvent::Error{..}     => return Ok(()),
        }
    }
    if tool_calls.is_empty() { app.emit("chat:done", ())?; return Ok(()); }

    // Append assistant message + tool result messages, then loop.
    working.push(/* assistant with tool_calls */);
    for part in &tool_calls { working.push(/* role: tool, content: execute_tool(...) */); }
}
```

Three lines (`pick`, `dispatch`, `recv`) is all it takes to wire laipe
into a Tauri app. The agent loop adds the optional multi-turn dance on
top. The `cancel` flag is checked each iteration so the cancel button
feels instant.

## Customization

The full step-by-step guide lives in [`EXTENDING.md`](../EXTENDING.md).
The short version:

### Add a tool

1. Add the schema to `src/tools.ts`:
   ```ts
   { type: "function", function: { name: "my_tool", description: "...", parameters: { ... } } }
   ```
2. Add the implementation to `src-tauri/src/lib.rs` `execute_tool`:
   ```rust
   "my_tool" => serde_json::json!({ "result": do_the_thing(args_json) }).to_string(),
   ```
3. Done. The frontend's `ToolCallCard` renders the call automatically.

### Add a system prompt per conversation

Edit `useConversations` (in `packages/laipe-vue/src/composables/useConversations.ts`)
to add a `systemPrompt?: string` field, and prepend a
`{ role: "system", content: conv.systemPrompt }` message in `App.vue`
before calling `useChat(tauriStream, TOOLS).send(...)`.

### Persist config to disk instead of localStorage

Replace `useConfig` (in `packages/laipe-vue/src/composables/useConfig.ts`)
with a Tauri command that writes to `app_data_dir()`. Frontend calls
`invoke('save_config', { cfg })` instead of `localStorage.setItem`.
See [Tauri 2 fs API](https://tauri.app/plugin/fs/).

### Add an icon tray + global hotkey

Tauri 2's `tray-icon` and `global-shortcut` plugins drop in cleanly. See
[tauri-apps/plugins-workspace](https://github.com/tauri-apps/plugins-workspace).

### Use Anthropic from the desktop

Same as the web — the format is configured in Settings. The Rust side
sends the right headers. **Note**: `anthropic-dangerous-direct-browser-access`
header is NOT used here because the request doesn't come from a browser —
Tauri acts as a native process, so Anthropic accepts the call normally.

### Ship to the App Store / Play Store

- **iOS**: requires an Apple Developer account ($99/yr), Xcode, and the
  matching provisioning profile. `cargo tauri build` produces an unsigned
  `.ipa`; you sign it with `xcodebuild` and upload via Transporter.
- **Android**: requires a Google Play Developer account ($25 one-time) and
  a keystore. `cargo tauri build` produces a signed `.aab` once you
  configure the keystore in `tauri.conf.json` under `bundle.android`.

## License

MIT — same as the rest of laipe.
