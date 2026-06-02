/**
 * Tokitai TypeScript client.
 *
 * A typed, zero-runtime-dependency client for the Tokitai MCP-compatible
 * HTTP server. Uses only Web APIs (`fetch`) so it runs in Node 18+,
 * browsers, Deno, Bun, and edge runtimes.
 *
 * Server endpoints (see docs/CROSS_LANGUAGE.md):
 *   GET  /tools       -> McpTool[]
 *   POST /call        -> ToolCallResponse
 *   POST /sse/<name>  -> Server-Sent Events (optional)
 */

// ---------------------------------------------------------------------------
// Types — strict shapes matching the Rust server's JSON.
// ---------------------------------------------------------------------------

export interface McpTool {
  readonly name: string;
  readonly description: string;
  readonly input_schema: Record<string, unknown>;
}

export interface ToolCallRequest {
  readonly name: string;
  readonly arguments: Record<string, unknown>;
}

export interface ToolCallResponse {
  readonly success: boolean;
  readonly result?: unknown;
  readonly error?: string;
}

export interface StreamEvent {
  readonly data: unknown;
  readonly event?: string;
  readonly id?: string;
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

export interface TokitaiClientOptions {
  /** Request timeout in milliseconds for unary calls. Default: 30 000. */
  readonly timeoutMs?: number;
  /** Custom fetch implementation (handy for tests / Cloudflare Workers). */
  readonly fetchImpl?: typeof fetch;
}

export class TokitaiError extends Error {
  constructor(message: string, readonly cause?: unknown) {
    super(message);
    this.name = "TokitaiError";
  }
}

export class SseNotSupportedError extends TokitaiError {
  constructor(baseUrl: string) {
    super(
      `Server ${baseUrl} does not expose SSE streaming. ` +
        "Rebuild tokitai-mcp-server with the 'sse' feature or fall back to callTool().",
    );
    this.name = "SseNotSupportedError";
  }
}

export class TokitaiClient {
  readonly baseUrl: string;
  private readonly timeoutMs: number;
  private readonly fetchImpl: typeof fetch;

  constructor(baseUrl: string, opts: TokitaiClientOptions = {}) {
    this.baseUrl = baseUrl.replace(/\/+$/, "");
    this.timeoutMs = opts.timeoutMs ?? 30_000;
    this.fetchImpl = opts.fetchImpl ?? fetch;
  }

  // -----------------------------------------------------------------
  // Discovery
  // -----------------------------------------------------------------
  async listTools(): Promise<McpTool[]> {
    const resp = await this.request("GET", "/tools");
    if (!resp.ok) {
      throw new TokitaiError(`listTools failed: ${resp.status} ${resp.statusText}`);
    }
    return (await resp.json()) as McpTool[];
  }

  // -----------------------------------------------------------------
  // Unary invocation
  // -----------------------------------------------------------------
  async callTool(name: string, args: Record<string, unknown> = {}): Promise<ToolCallResponse> {
    const body: ToolCallRequest = { name, arguments: args };
    const resp = await this.request("POST", "/call", body);
    if (!resp.ok) {
      throw new TokitaiError(`callTool failed: ${resp.status} ${resp.statusText}`);
    }
    return (await resp.json()) as ToolCallResponse;
  }

  // -----------------------------------------------------------------
  // Streaming (SSE)
  // -----------------------------------------------------------------
  /**
   * Subscribe to the SSE stream for `name`. The server's response must
   * use `text/event-stream`; otherwise we throw `SseNotSupportedError`
   * so callers fail fast instead of buffering forever.
   *
   * Implemented with `fetch` + a local SSE parser, so it works in any
   * environment that ships a global `fetch` (Node 18+, browsers, Deno,
   * Bun) — no `EventSource` polyfill is required.
   */
  async *streamTool(
    name: string,
    args: Record<string, unknown> = {},
  ): AsyncGenerator<StreamEvent> {
    const resp = await this.request("POST", `/sse/${encodeURIComponent(name)}`, {
      name,
      arguments: args,
    });
    if (resp.status === 404) {
      throw new SseNotSupportedError(this.baseUrl);
    }
    if (!resp.ok || !resp.body) {
      throw new TokitaiError(`streamTool failed: ${resp.status} ${resp.statusText}`);
    }

    yield* parseSseStream(resp.body);
  }

  // -----------------------------------------------------------------
  // Internals
  // -----------------------------------------------------------------
  private request(
    method: string,
    path: string,
    body?: unknown,
  ): Promise<Response> {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeoutMs);
    const init: RequestInit = {
      method,
      headers: body !== undefined ? { "content-type": "application/json" } : undefined,
      body: body !== undefined ? JSON.stringify(body) : undefined,
      signal: controller.signal,
    };
    return this.fetchImpl(`${this.baseUrl}${path}`, init).finally(() =>
      clearTimeout(timer),
    );
  }
}

// ---------------------------------------------------------------------------
// SSE parsing
// ---------------------------------------------------------------------------

/**
 * Minimal SSE parser operating on a WHATWG ReadableStream. Yields one
 * `StreamEvent` per `data:` block. Multi-line `data:` fields are joined
 * with newlines per the SSE spec.
 */
export async function* parseSseStream(
  body: ReadableStream<Uint8Array>,
): AsyncGenerator<StreamEvent> {
  const decoder = new TextDecoder("utf-8");
  const reader = body.getReader();
  let buffer = "";
  let dataLines: string[] = [];
  let eventName: string | undefined;
  let id: string | undefined;

  const dispatch = (): StreamEvent | undefined => {
    if (dataLines.length === 0 && !eventName && !id) return undefined;
    const raw = dataLines.join("\n");
    let parsed: unknown = raw;
    if (raw.length > 0) {
      try {
        parsed = JSON.parse(raw);
      } catch {
        // Keep as raw string.
      }
    }
    const evt: StreamEvent = { data: parsed, event: eventName, id };
    dataLines = [];
    eventName = undefined;
    id = undefined;
    return evt;
  };

  try {
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      let nl: number;
      while ((nl = buffer.indexOf("\n")) >= 0) {
        const line = buffer.slice(0, nl).replace(/\r$/, "");
        buffer = buffer.slice(nl + 1);
        if (line === "") {
          const evt = dispatch();
          if (evt) yield evt;
          continue;
        }
        if (line.startsWith(":")) continue; // comment
        const colon = line.indexOf(":");
        const field = colon === -1 ? line : line.slice(0, colon);
        let val = colon === -1 ? "" : line.slice(colon + 1);
        if (val.startsWith(" ")) val = val.slice(1);
        switch (field) {
          case "data":
            dataLines.push(val);
            break;
          case "event":
            eventName = val;
            break;
          case "id":
            id = val;
            break;
        }
      }
    }
    const tail = dispatch();
    if (tail) yield tail;
  } finally {
    reader.releaseLock();
  }
}
