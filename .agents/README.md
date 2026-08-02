# `.agents/` — LLM-facing knowledge

This folder is the **single source of knowledge for AI coding agents** (LLMs, Copilot, Cursor, Claude Code, etc.) working on or with laipe.

If you're an LLM picking up this codebase, **read this file first**, then follow the navigation order below. The order is designed to give you the most context for the fewest tokens.

## Navigation order

| # | File | Why read it |
|---|---|---|
| 1 | [`AGENTS.md`](AGENTS.md) | Project conventions: pluggability + LLM-friendliness + code style + testing + security |
| 2 | [`API.md`](API.md) | Single source of truth for the public API surface. Start here for "where is X defined / what calls Y". |
| 3 | [`ARCHITECTURE.md`](ARCHITECTURE.md) | Crate boundaries, streaming pipeline, anti-stutter, **pluggability seams map** |
| 4 | [`EXTENDING.md`](EXTENDING.md) | Fork-and-extend guide: 10-step walkthrough + 2 worked examples + **LLM-assisted development pointers** |
| 5 | [`VISION.md`](VISION.md) | One-line positioning, what laipe is and isn't, target users |
| 6 | [`ROADMAP.md`](ROADMAP.md) | v0.1 → v1.0 plan, what's deliberately deferred |
| 7 | [`CHANGELOG.md`](CHANGELOG.md) | Per-version what changed |
| 8 | [`CONTRIBUTING.md`](CONTRIBUTING.md) | Commit message style, ground rules |
| 9 | [`docs/PROTOCOLS.md`](docs/PROTOCOLS.md) | 3-protocol comparison table + when to pick which |
| 10 | [`docs/STREAMING.md`](docs/STREAMING.md) | 4 anti-stutter countermeasures + StreamEvent flow |
| 11 | [`docs/TOOL_CALLING.md`](docs/TOOL_CALLING.md) | Tool schema cross-protocol translation table + 3 built-in patterns |

## File layout

```
.agents/
├── README.md          ← this file (the index)
├── AGENTS.md           ← project conventions
├── API.md              ← public API reference
├── ARCHITECTURE.md     ← architecture + pluggability seams map
├── EXTENDING.md        ← fork-and-extend guide
├── VISION.md           ← positioning
├── ROADMAP.md          ← version plan
├── CHANGELOG.md        ← per-version changes
├── CONTRIBUTING.md     ← contribution rules
└── docs/
    ├── PROTOCOLS.md    ← protocol comparison
    ├── STREAMING.md    ← streaming pipeline
    └── TOOL_CALLING.md ← tool calling flow
```

## How to use this folder

- **As an LLM**: read in the order above. When the user asks for a feature, jump to the relevant section in `EXTENDING.md` (most "how do I…" questions have a code-level answer there).
- **As a human developer**: same as before — these files used to live at the repo root and inside `docs/`. They still work, just in a unified folder.
- **As a tool author**: tools that auto-detect `AGENTS.md` should look in `.agents/AGENTS.md` first, fall back to the root.

## See also

- [Root `README.md`](../../README.md) — top-level pitch + quickstart
- [Per-package READMEs](../../laipe-app/README.md), [laipe-ts](../../packages/laipe-ts/README.md), [laipe-vue](../../packages/laipe-vue/README.md) — package-level documentation
