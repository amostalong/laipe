# Contributing

`laipe` is currently maintained by [@amostalong](https://github.com/amostalong) as a side project. The project is in **v0.1.1** (protocol layer skeleton), and the maintainer is the only regular user so far. Contributions are welcome, but the project is **not** set up for casual open-source contribution yet — read this first.

## Status: pre-community

Until v1.0:
- The public API is **not stable**. Every v0.x minor release may rename or reshape public types.
- There's no CI yet. `cargo check --workspace` + `cargo test --workspace` is the de-facto gate; the maintainer runs them locally before each release.
- There's no issue tracker workflow yet. Open issues are fine, but expect the maintainer to triage ad-hoc.
- There's no PR template. If you open one, just describe what you changed and why.

After v0.5 (when CI lands and the API starts settling): this file will be rewritten with a real contribution guide, issue templates, and a PR template.

## Ground rules (always)

1. **`cargo check --workspace` and `cargo test --workspace` must be green** before you push or open a PR. No exceptions.
2. **One logical change per commit**. Don't bundle "fix typo in README" with "rewrite the SSE parser".
3. **Match the existing style** — `rustfmt.toml` + `clippy.toml` are checked in. Run `cargo fmt` before committing.
4. **Add a test** for any new behavior. The 25 existing tests in `laipe-streaming` are the floor; new public API needs new tests.
5. **Document the why, not just the what** — commit messages and code comments should explain the reasoning, not just restate the diff.

## How to make a change (today)

```bash
# 1. Fork or clone
git clone https://github.com/amostalong/laipe.git
cd laipe

# 2. Make a branch
git checkout -b fix/<one-word-summary>

# 3. Edit, test, format
$EDITOR crates/laipe-streaming/src/...
cargo test -p laipe-streaming
cargo fmt

# 4. Commit
git add -p
git commit -F $TMP/msg.txt   # see commit message style below

# 5. Push and open a PR
git push origin fix/<one-word-summary>
```

## Commit message style

We use the [Conventional Commits](https://www.conventionalcommits.org/) style, lightly:

```
<type>: <short summary>

<optional body — explain the why, not the what>

<optional footer — refs, breaking changes>
```

`<type>` is one of:
- `feat` — new public API
- `fix` — bug fix
- `refactor` — internal change, no public API change
- `docs` — docs only (README, docs/, comments)
- `test` — tests only
- `chore` — tooling, deps, configs
- `perf` — performance improvement

`<short summary>` — under 70 chars, no period, present tense ("add", not "added").

Examples from the v0.1 history:
- `feat: laipe v0.1.1 — 3-protocol LLM streaming agent framework`
- `refactor: extract SseParser to crate::sse for shared use across protocols`

For multi-line messages, write to a file and use `git commit -F`:

```bash
# PowerShell 5.1 — single-quote subject, escape inner quotes
@"
feat: add Anthropic prompt caching

Caches the system prompt and the last 4 messages; reduces token
cost by 60% on long conversations.

Refs: https://docs.anthropic.com/...
"@ | Set-Content -Encoding utf8 $env:TEMP/msg.txt
git commit -F $env:TEMP/msg.txt
```

## What to work on

If you want to help but don't know where to start, here are the highest-leverage items (most → least impactful):

1. **`laipe-tokio::run_to_completion_throttled`** — the 16ms rAF + 256-char batch throttling helper. v0.2 work, no public API change needed. See `docs/STREAMING.md` for the spec.
2. **`examples/tauri-minimal`** — a Tauri 2 desktop app with 1 chat tab + 1 example tab. v0.2 work. Lets new users see laipe work in 5 minutes.
3. **Multi-round tool calling** — `laipe-streaming` currently handles 1 round of tool calls; round 2 (assistant `tool_calls` → tool result → re-request) needs explicit consumer glue. Either build the helper into `laipe-streaming` or document the consumer pattern in `docs/TOOL_CALLING.md`.
4. **GLM `reasoning_content` pass-through** — ZhipuGLM's `/v1/chat/completions` returns a `reasoning_content` field in addition to `content`. v0.2 work; small change in `openai_chat.rs`.
5. **CI** — GitHub Actions running `cargo test --workspace` on PR. v0.3 work. Not glamorous but unblocks every other contributor.
6. **`packages/laipe-ts`** — the frontend mirror of `laipe-core` types. v0.2 work. Pure types + `fetchSSE()` helper.

## What NOT to do (today)

- **Don't add new public API surface without first opening an issue** to discuss. The v0.x API is small on purpose; we'd rather turn down 5 PRs than ship a v0.2 with 5 features that don't fit together.
- **Don't add new third-party dependencies** without a strong reason. The 11 deps in `Cargo.toml` are the floor; new deps multiply the audit surface.
- **Don't add async-trait-free async fn in trait** — `async_trait` is in use, all public traits are `#[async_trait]`. Switching to native AFIT mid-v0 is a breaking change.
- **Don't run `cargo update` on the workspace** unless you're explicitly trying to fix a security issue. The lockfile is pinned.

## License

By contributing, you agree that your contributions are licensed under the
project's MIT license (see `LICENSE`).
