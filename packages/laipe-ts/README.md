# laipe-ts

TypeScript mirror of [`laipe-core`](../../crates/laipe-core) + a 3-protocol SSE
streaming client (`dispatchStream` + `SseParser`). Browser-friendly,
Tauri-webview friendly, Node-script friendly.

This is the v0.2 deliverable. Pure types + a tiny runtime. No framework, no UI
— [`packages/laipe-vue`](../laipe-vue) builds on top of this.

## Install

In a workspace that has `laipe-ts` as a dependency:

```ts
import { dispatchStream, type ChatMessage, type ProviderConfig, type StreamEvent } from "laipe-ts";
```

## Quick start

```ts
import { dispatchStream, ChatStreamError } from "laipe-ts";

const config: ProviderConfig = {
  endpoint: "https://api.openai.com/v1",
  api_key: process.env.OPENAI_API_KEY!,
  model: "gpt-4o-mini",
  api_format: "openai_chat",
};

const messages: ChatMessage[] = [
  { role: "user", content: "Say hello in 5 words or less." },
];

try {
  for await (const ev of dispatchStream(config, messages)) {
    if (ev.type === "text") process.stdout.write(ev.delta);
    else if (ev.type === "done") break;
    else if (ev.type === "error") console.error(`[${ev.kind}] ${ev.message}`);
  }
} catch (e) {
  if (e instanceof ChatStreamError) {
    console.error(`pre-stream error [${e.kind}] status=${e.status}: ${e.message}`);
  } else {
    throw e;
  }
}
```

## Cancellation

```ts
const ac = new AbortController();
const task = (async () => {
  for await (const ev of dispatchStream(config, messages, undefined, { signal: ac.signal })) {
    // ...
  }
})();

setTimeout(() => ac.abort(), 5000);
```

Signature: `dispatchStream(config, messages, tools?, options?)` where `options = { signal?: AbortSignal }`. The 3rd arg is tools (for tool-calling protocols), the 4th is the options bag with the abort signal.
```

## Supported protocols

| `api_format`     | Status      | Endpoint path                |
|------------------|-------------|------------------------------|
| `openai_chat`    | ✅ full     | `{endpoint}/chat/completions`|
| `openai_responses`| ✅ full    | `{endpoint}/responses`       |
| `anthropic`      | ✅ full     | `{endpoint}/v1/messages`     |

All three share the same `SseParser` byte parser — see `src/sse.ts`. Differences
are only in request body shape and event name → `StreamEvent` mapping.

## Type ↔ Rust mapping

Every Rust type in `crates/laipe-core/src/` has a TS mirror in `src/types.ts`
and `src/errors.ts`. Wire format keeps snake_case to match the Rust serde
renames; TS-side identifiers stay camelCase. The discriminator on
`StreamEvent` is `type: 'text' | 'tool_calls' | 'done' | 'error'` (camelCase
identifiers, not the Rust CamelCase variants).
