// Error classification + custom error class for laipe-ts.
// Mirrors `laipe_core::ChatErrorKind` 1:1.

import type { ChatErrorKind } from "./types.js";

/** Map an HTTP status code to the closest `ChatErrorKind`. */
export function errorKindFromStatus(status: number): ChatErrorKind {
  if (status === 401 || status === 403) return "auth";
  if (status === 404) return "model_not_found";
  if (status === 429) return "rate_limit";
  if (status >= 500 && status <= 599) return "server_error";
  if (status >= 400 && status <= 499) return "bad_request";
  return "unknown";
}

/**
 * Thrown by `dispatchStream` for errors that happen *before* the stream opens
 * (e.g. 401 from the upstream). Mid-stream errors are yielded as
 * `{ type: "error", ... }` events instead, matching the Rust contract.
 */
export class ChatStreamError extends Error {
  readonly kind: ChatErrorKind;
  readonly status?: number;
  readonly body?: string;

  constructor(kind: ChatErrorKind, message: string, opts: { status?: number; body?: string } = {}) {
    super(message);
    this.name = "ChatStreamError";
    this.kind = kind;
    this.status = opts.status;
    this.body = opts.body;
  }
}
