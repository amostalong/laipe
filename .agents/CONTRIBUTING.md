# Contributing

`laipe` is currently maintained by [@amostalong](https://github.com/amostalong) as a personal project. The project is in **v0.2** — stack-locked starter (Rust + Vue 3 + Tauri 2) for building agent client desktop apps. The maintainer is the only regular user so far. Contributions are welcome, but the project is **not** set up for casual open-source contribution yet — read this first.

## Status: pre-community

Until v1.0:
- The public API is **not stable**. Every v0.x minor release may rename or reshape public types.
- There's no CI yet. `bun run gates` is the de-facto gate; the maintainer runs it locally before each release.
- There's no issue tracker workflow yet. Open issues are fine, but expect the maintainer to triage ad-hoc.
- There's no PR template. If you open one, just describe what you changed and why.

After v0.5 (when CI lands and the API starts settling): this file will be rewritten with a real contribution guide, issue templates, and a PR template.

## Ground rules (always)

1. **`bun run gates` must be green** before you push or open a PR. No exceptions. The gates are:
   - `cargo fmt --check` (no diffs)
   - `cargo clippy --workspace --all-targets -- -D warnings` (zero warnings)
   - `cargo test --workspace` (all tests pass)
   - `bun run typecheck:ts` (zero errors across all TS packages + laipe-app)
   - `bun run build:app-fe` (Vite build of laipe-app succeeds)
   - `cargo check -p laipe-app` (Tauri 2 backend compiles)

2. **One logical change per commit**. Don't bundle "fix typo" with "rewrite parser".

3. **Match the existing style**:
   - Rust: `rustfmt.toml` (100-wide, 4-tab) + `clippy.toml` (stricter than default) + workspace lints in `Cargo.toml [workspace.lints]`
   - TypeScript / Vue: `tsconfig.json` (strict, `noUncheckedIndexedAccess`), scoped CSS, Composition API with `<script setup lang="ts">`

4. **Add a test** for any new behavior:
   - Rust: add `#[test]` in a `#[cfg(test)] mod tests` next to your code
   - Vue (v0.3+): vitest + vue-test-utils

5. **Document the *why*, not just the *what*** — commit messages and code comments should explain the reasoning, not just restate the diff.

## How to make a change (today)

```bash
# 1. Fork or clone
git clone https://github.com/amostalong/laipe.git
cd laipe

# 2. Make a branch
git checkout -b fix/<one-word-summary>

# 3. Edit
$EDITOR crates/laipe-streaming/src/...

# 4. Test + format
bun run gates

# 5. Commit
git add -p
git commit -F $TMP/msg.txt   # see commit message style below

# 6. Push and open a PR
git push origin fix/<one-word-summary>
```

## Where things live

| Path | What you'll find |
|---|---|
| `crates/laipe-core/` | Protocol-agnostic types (`ChatMessage`, `StreamEvent`, `ChatErrorKind`, tool schema). **No HTTP / async deps.** Any change here ripples to all 3 protocol impls. |
| `crates/laipe-streaming/` | 3 protocol implementations + shared `sse::SseParser`. **Add a new protocol here** when needed. |
| `crates/laipe-tokio/` | `CancelHandle` + `run_to_completion`. Tiny helper crate. |
| `packages/laipe-ts/` | TypeScript SDK. **Mirror the laipe-core API surface.** Any type added to laipe-core must be mirrored here. |
| `packages/laipe-vue/` | Vue 3 components. 3 layers (primitives / composites / batteries-included). |
| `laipe-app/` | Tauri 2 starter app. The deliverable. |
| `docs/` | Design + spec docs. Update when behavior changes. |

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

For multi-line messages, write to a file and use `git commit -F`:

```bash
# PowerShell 5.1: single-quote the subject, escape inner quotes
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

1. **`laipe-streaming` — multi-round tool calling** — the v0.1 layer handles 1 round of tool calls; round 2 (assistant `tool_calls` → tool result echo → re-request) needs explicit consumer glue. Either build the helper into `laipe-streaming` or document the consumer pattern in `docs/TOOL_CALLING.md`.
2. **`packages/laipe-vue` — test suite** — v0.3 work. vitest + vue-test-utils. The `StreamSource` injection makes this easy: pass a `mockStream` in tests.
3. **`laipe-app` — OS keyring** — replace the `localStorage` API-key fallback with `tauri-plugin-stronghold` for production-quality secret storage.
4. **CI** — GitHub Actions running `bun run gates` on every PR. v0.3 work. Not glamorous but unblocks every other contributor.
5. **`packages/laipe-vue` — default message actions** — copy / regenerate / edit buttons in `MessageBubble`'s `actions` slot. Add a `<MessageActions :message="m" />` primitive.
6. **Mobile signing docs** — `laipe-app/src-tauri/tauri.conf.json` already lists all 5 platforms; document the iOS App Store / Google Play submission flow.

## What NOT to do (today)

- **Don't add a new public API surface** without first opening an issue to discuss. The v0.x API is small on purpose; we'd rather turn down 5 PRs than ship a v0.3 with 5 features that don't fit together.
- **Don't add new third-party dependencies** without a strong reason. The 11 deps in `Cargo.toml` and 6 in the bun workspace are the floor; new deps multiply the audit surface.
- **Don't add native AFIT** — `async_trait` is in use, all public traits are `#[async_trait]`. Switching to native AFIT mid-v0 is a breaking change.
- **Don't run `cargo update` on the workspace** unless you're explicitly fixing a security issue. The lockfile is pinned.
- **Don't add a non-Tauri or non-Vue alternative** — the stack is fixed by design. If you want React, fork laipe-app and replace the Vue layer.
- **Don't bundle "fix typo" with "rewrite parser"** — one logical change per commit.

## License

By contributing, you agree that your contributions are licensed under the project's MIT license (see [LICENSE](LICENSE)).
