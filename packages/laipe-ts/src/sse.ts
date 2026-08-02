// Minimal SSE byte parser. Shared by all three protocol streams.
// Handles `\n\n` and `\r\n\r\n` frame terminators, comments (`: ping`),
// and `event:` / `data:` fields. Mirrors the logic in
// `crates/laipe-streaming/src/sse.rs::SseParser` so behavior is identical.

export interface SseFrame {
  /** Value from the last `event:` line, if any. */
  event?: string;
  /** Joined `data:` lines (separated by `\n`). */
  data: string;
}

export class SseParser {
  private buf = "";

  /** Feed a chunk of decoded text, return any complete frames. */
  feed(chunk: string): SseFrame[] {
    this.buf += chunk;
    const out: SseFrame[] = [];
    while (true) {
      // Accept \n\n or \r\n\r\n
      let idx = this.buf.indexOf("\n\n");
      let skip = 2;
      if (idx === -1) {
        idx = this.buf.indexOf("\r\n\r\n");
        skip = 4;
      }
      if (idx === -1) break;

      const raw = this.buf.slice(0, idx);
      this.buf = this.buf.slice(idx + skip);
      const frame = parseFrame(raw);
      if (frame) out.push(frame);
    }
    return out;
  }
}

function parseFrame(raw: string): SseFrame | null {
  const lines = raw.split(/\r?\n/);
  let event: string | undefined;
  const data: string[] = [];
  for (const line of lines) {
    if (!line || line.startsWith(":")) continue; // empty / comment
    const colon = line.indexOf(":");
    if (colon === -1) continue;
    const field = line.slice(0, colon);
    let value = line.slice(colon + 1);
    if (value.startsWith(" ")) value = value.slice(1);
    if (field === "event") event = value;
    else if (field === "data") data.push(value);
  }
  if (data.length === 0 && !event) return null;
  return { event, data: data.join("\n") };
}
