# Cross-Language SDK Guide

**Version**: 0.5.0
**Last updated**: 2026-06-02

The HTTP+JSON contract spoken by the `tokitai-mcp-server` crate,
and how to call it from Python, JavaScript / TypeScript, Go, and
plain `curl`. Reference client implementations live under
`examples/{py,js,go,curl}/`; their public API shapes are stable
across the 0.5.x release line.

---

## 1. Protocol Overview

Tokitai's MCP server speaks a small, plain HTTP+JSON contract on
top of `tokio::hyper`. It is **not** the full JSON-RPC 2.0 spec
used by some MCP implementations; it is the de-facto "MCP-style"
subset that exposes tools over an ordinary request/response API.

| Method | Path | Purpose |
| --- | --- | --- |
| `GET`  | `/tools`  | Return every registered tool's metadata. |
| `POST` | `/call`   | Invoke a single tool by name. |
| `GET`  | `/health` | Liveness / readiness probe. |

A fourth endpoint, `POST /sse/<name>`, is feature-gated and
returns `text/event-stream`; see section 7. The server is
`Send + Sync` and built on `tokio`. Multiple `/call` requests
are dispatched concurrently to the underlying `#[tool]`
implementations, even if those implementations are blocking.
There is no shared mutable state between requests beyond the
immutable provider set, so clients can safely issue hundreds
of concurrent `POST /call` requests against a single server.

### 1.1 Wire format

`GET /tools` returns a JSON array of `McpTool` objects, each
carrying a `name`, human-readable `description`, and a JSON
Schema (`input_schema`) for its arguments. Tool metadata is
generated at compile time by the `#[tool]` macro, so the schema
is always in sync with the Rust signature.

```bash
$ curl -s http://127.0.0.1:8080/tools | jq '.[0]'
{
  "name": "add",
  "description": "Add two numbers together",
  "input_schema": {
    "type": "object",
    "properties": { "a": {"type":"integer"}, "b": {"type":"integer"} },
    "required": ["a", "b"]
  }
}
```

`POST /call` accepts a `ToolCallRequest` body and returns a
`ToolCallResponse` body. On tool-level failure the response is
still HTTP 200 but carries `success: false` and a string
`error` field. HTTP 4xx/5xx is reserved for protocol-level
failures — see section 5.

```bash
$ curl -s -X POST http://127.0.0.1:8080/call \
    -H 'content-type: application/json' \
    -d '{"name":"add","arguments":{"a":1,"b":2}}'
{ "success": true, "result": 3 }
```

---

## 2. Endpoint Reference

### 2.1 `GET /tools`

Returns every tool the server has registered. The response is a
JSON array of `McpTool` objects with the shape
`{ name, description, input_schema }` shown above.

**When to call it.** Call `GET /tools` exactly once, on client
startup, after constructing your `TokitaiClient`. The response
rarely changes for the lifetime of the server, so cache it for
the duration of the process. Re-call only on server restart or
when the tool set is known to have changed. Caching clients can
also use the `ETag` and `Cache-Control` headers the server emits.

### 2.2 `POST /call`

Invokes a single tool and returns its result. The request is a
`ToolCallRequest` (`name: string`, `arguments: object` — pass
`{}` for tools that take no parameters). The response is a
`ToolCallResponse` with `success: bool`, plus either `result` on
success or `error: string` on tool failure. HTTP status codes:

- `200` — request was well-formed; inspect `success` to know
  whether the tool itself succeeded.
- `400` — body could not be parsed, or `name` / `arguments` were
  missing.
- `404` — the `name` is not registered, or the path is unknown.
- `405` — wrong method (e.g. `GET` on `/call`).
- `413` — request body exceeded the configured limit (default
  1 MiB).
- `500` — tool panicked or an internal step failed.
- `503` — server is shutting down.

### 2.3 `GET /health`

Returns the literal string `"ok"` with `Content-Type: text/plain`
and HTTP 200. Suitable as a Kubernetes liveness / readiness
probe. Always cheap (no allocation, no JSON).

### 2.4 Future / planned endpoints

Tracked on the roadmap but **not** part of the 0.5.0 contract:
`POST /sse/<name>` (SSE; opt-in via the `sse` build feature —
see section 7), `WebSocket /ws` (bidirectional), and
`POST /batch` (multi-call in one round-trip).

---

## 3. Cross-Language Quickstart

Each example below assumes the server in section 4 is running
on `http://127.0.0.1:8080`.

### 3.1 Python

Async client, built on `httpx`, lives in
`examples/py/tokitai_client.py`. Requires Python 3.9+.

**Install** (from a checkout):

```bash
cd examples/py && pip install -e .
# or, with proxy support:
pip install "tokitai-client[socks]"
```

**Complete 20-line script**:

```python
import asyncio
from tokitai_client import TokitaiClient

async def main() -> None:
    async with TokitaiClient("http://127.0.0.1:8080") as client:
        # 1. Discover tools (once, on startup)
        tools = await client.list_tools()
        for t in tools:
            print(f"- {t.name}: {t.description}")

        # 2. Call the canonical add(1, 2) example
        resp = await client.call_tool("add", {"a": 1, "b": 2})
        if resp["success"]:
            print(f"add(1, 2) = {resp['result']}")  # 3
        else:
            print(f"tool error: {resp['error']}")

        # 3. (optional) stream from an SSE-capable server
        try:
            async for chunk in client.stream_tool("add", {"a": 4, "b": 5}):
                print("chunk:", chunk)
        except NotImplementedError as e:
            print("SSE not available:", e)

asyncio.run(main())
```

`TokitaiClient` is a context manager; use `async with` to ensure
the underlying `httpx.AsyncClient` closes cleanly.

### 3.2 JavaScript / TypeScript

The JS client has **zero runtime dependencies** — only Web APIs
(`fetch`, `EventSource`) — and runs unchanged in Node 18+,
browsers, Deno, and Bun. Lives in
`examples/js/tokitai-client.ts`.

**Install**: `cd examples/js && npm install`.

**Complete 20-line script** (`src/example.ts`):

```ts
import { TokitaiClient } from "./tokitai-client.js";

async function main(): Promise<void> {
  const client = new TokitaiClient("http://127.0.0.1:8080");
  console.log("Tokitai TypeScript client example");

  const tools = await client.listTools();
  console.log(`Server exposes ${tools.length} tool(s):`);
  for (const tool of tools) {
    console.log(`  - ${tool.name}: ${tool.description}`);
  }

  const resp = await client.callTool("add", { a: 1, b: 2 });
  if (resp.success) {
    console.log(`add(1, 2) = ${JSON.stringify(resp.result)}`);
  } else {
    console.error(`tool error: ${resp.error}`);
    process.exitCode = 1;
  }
}

main().catch((err) => { console.error("fatal:", err); process.exit(1); });
```

Run with `npm start`. Pass a custom `fetchImpl` in
`TokitaiClientOptions` to inject a Cloudflare Workers or test
fake.

### 3.3 Go

Standard-library only (`net/http`, `encoding/json`, `bufio`).
Safe for concurrent use across goroutines; each call takes its
own `context.Context`. Lives in `examples/go/tokitai.go`.

**Module setup** — `examples/go/go.mod`:

```go
module github.com/silverenternal/tokitai/examples/go
go 1.21
```

**Build**: `cd examples/go && go build ./...`

**Complete 20-line program** (`cmd/call-add/main.go`):

```go
package main

import (
	"context"
	"fmt"
	"log"
	"time"

	"github.com/silverenternal/tokitai/examples/go/tokitai"
)

func main() {
	client := tokitai.New("http://127.0.0.1:8080")
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	tools, err := client.ListTools(ctx)
	if err != nil { log.Fatal(err) }
	for _, t := range tools {
		fmt.Printf("- %s: %s\n", t.Name, t.Description)
	}

	resp, err := client.CallTool(ctx, "add", map[string]any{"a": 1, "b": 2})
	if err != nil { log.Fatal(err) }
	if resp.Success {
		fmt.Printf("add(1, 2) = %s\n", string(resp.Result))
	} else {
		fmt.Printf("tool error: %s\n", resp.Error)
	}
}
```

The client also exposes `Client.StreamCall(ctx, name, args)` and
returns a sentinel `ErrSseNotSupported` when SSE is disabled —
branch on `errors.Is(err, tokitai.ErrSseNotSupported)` and fall
back to `CallTool`.

### 3.4 curl

The `examples/curl/` directory ships three small shell scripts
that wrap `curl` and `jq`. **Requirements**: `bash`, `curl`,
`jq`.

| Script | What it does |
| --- | --- |
| `list-tools.sh`  | `GET /tools` and pretty-print the response. |
| `call-tool.sh`   | `POST /call` with a name and JSON arguments. |
| `stream-tool.sh` | `POST /sse/<name>` with `curl -N` for live output. |

```bash
# List tools (override host with BASE_URL=...)
./list-tools.sh

# Call a tool
NAME=add ARGS_JSON='{"a":1,"b":2}' ./call-tool.sh
NAME=to_uppercase ARGS_JSON='{"text":"hello"}' ./call-tool.sh

# Stream from a tool (server must have the `sse` feature)
NAME=add ARGS_JSON='{"a":10,"b":32}' ./stream-tool.sh
```

All three scripts honour `BASE_URL`, so they work unchanged
against staging or production servers.

---

## 4. Running the Server

The reference HTTP server used throughout this document lives in
`tokitai-mcp-server/examples/mcp_builder_demo.rs`. It exposes a
`Calculator` and a `TextTools` group, and binds to
`http://127.0.0.1:8080` by default.

**Start it**:

```bash
cargo run -p tokitai-mcp-server --example mcp_builder_demo
```

The demo prints its registered tools, then waits for `Enter`
before starting the HTTP listener. Once it prints
`listening on 127.0.0.1:8080`, the server is ready.

**Override the bind address** at the source level via the
`McpServerBuilder` chain
(`.with_host("0.0.0.0")`, `.with_port(8080)`,
`.with_cors(true)`).

**Health check**:

```bash
$ curl -s http://127.0.0.1:8080/health
OK
```

Use this in your orchestrator's readiness probe. Stop the server
with `Ctrl+C`; the `tokio::signal` handler performs a graceful
drain of in-flight requests before exiting.

---

## 5. Error Handling

Tokitai distinguishes **protocol-level** errors (the wire format
is broken) from **tool-level** errors (the wire format was fine
but the tool itself failed). The first surface as HTTP 4xx/5xx;
the second surface as HTTP 200 with `success: false`.

### 5.1 When does the server return `success: false`?

The server returns `200 OK` with
`{ "success": false, "error": "..." }` whenever the request was
syntactically valid but the tool could not produce a result: a
required argument is missing or wrong-typed; the tool returned
`Result::Err`; or the tool panicked and the panic hook caught it.
These map to `ToolErrorKind` in `tokitai-core`. (`NotFound` is the
one exception — it is surfaced as protocol-level `HTTP 404`
without a JSON body, as documented in section 2.2.)

| Kind | Meaning | Sample `error` |
| --- | --- | --- |
| `ValidationError` | Argument failed JSON Schema validation. | `"missing field: a"` |
| `NotFound`        | The named tool is not registered. | `HTTP 404` (no `success:false` body) |
| `TypeError`       | Argument failed to coerce to the Rust type. | `"expected integer, got string"` |
| `InternalError`   | The tool returned an error or panicked. | `"internal error: division by zero"` |

Clients can branch on the prefix of the `error` string or just
display it to the user — it is meant to be human-readable.

### 5.2 Idempotency and retry guidance

**A `POST /call` is not necessarily idempotent.** Many tools are
pure (`add`, `to_uppercase`, `sqrt`), but others have side
effects (`append_to_log`, `send_email`, `increment_counter`).
Recommended retry policy:

- `success: true` — done. No retry.
- `success: false` with `ValidationError` / `NotFound` /
  `TypeError` — fix the inputs and re-issue. **Do not blindly
  retry** — the same input will produce the same failure.
- `success: false` with `InternalError` — *consider* retrying
  once after a small backoff, but think first: the tool may
  have had side effects. For tools that wrap external side
  effects, treat `InternalError` as "I don't know if it worked;
  check before retrying."
- `500` / `503` — retry with exponential backoff (100 ms,
  400 ms, 1.6 s, capped at 5 s), with jitter, max three
  attempts.
- `400` / `404` / `405` / `413` — do **not** retry.

A future minor version may add an `Idempotency-Key` header; it
is not part of the 0.5.0 contract yet.

---

## 6. Authentication

**Tokitai's HTTP server has no built-in authentication.** There
is no API key, no bearer token, no OAuth flow, no per-tool ACL.
This is intentional: the server is a low-level building block
designed to run inside a trusted network or behind a reverse
proxy. **Do not expose port 8080 to the public Internet without
putting an authenticating proxy in front of it.**

Recommended topology:

```
Internet ──> Caddy / nginx / Traefik ──> 127.0.0.1:8080 (Tokitai)
```

The reverse proxy handles TLS termination, request auth (mTLS,
OIDC, basic auth, Cloudflare Access — your choice), rate
limiting, and audit logging. Tokitai only sees a plain
`HTTP/1.1` request from `127.0.0.1`.

**One-line Caddy example** (`Caddyfile`):

```caddy
api.example.com {
    reverse_proxy 127.0.0.1:8080
    basicauth {
        ai_user JDJhJDE0JDdGcFhYVko0d3pGaW9CLzJZb0RZb1hXUjJkT2x0YU5xT2tTQ0Z
    }
}
```

When a stronger threat model is needed (multi-tenant SaaS,
HIPAA, etc.), put the proxy in a sealed VPC and require mTLS at
the load balancer.

---

## 7. Streaming / Long-Running Tools

The 0.5.0 release ships a **feature-gated** SSE endpoint at
`POST /sse/<name>`. When the server is built with the `sse`
feature, that endpoint returns `text/event-stream`, with each
`data:` line carrying a JSON fragment of the tool's incremental
output.

This endpoint is opt-in and is not part of the minimum contract.
Clients should probe for it and degrade gracefully — a 404 from
`HEAD /sse/<name>` means the server was built without SSE and
you should fall back to `POST /call`. All three reference SDKs
implement this fallback: Python raises `NotImplementedError`,
TypeScript throws `SseNotSupportedError`, Go returns the
sentinel `ErrSseNotSupported`.

### 7.1 Workarounds for a non-SSE server

If you can't enable SSE, there are two common patterns for
long-running tools:

**Start / poll.** Split the tool into a pair:
`start_long_job(args) -> job_id` and
`poll_long_job(job_id) -> status`. `start_long_job` is
non-blocking; `poll_long_job` is a regular unary call. The
client polls every N seconds until the status reaches
`completed` or `failed`.

**Chunked response.** Encode the tool's output as a JSON array
and return it in one `POST /call` response. This works up to a
few megabytes; beyond that, the 1 MiB body limit on `/call`
will start to bite.

### 7.2 Roadmap

Tracked for a future 0.5.x release: a stable, non-gated SSE
endpoint with a documented event schema (`event: progress`,
`event: data`, `event: end`); a WebSocket transport (`/ws`) for
bidirectional streaming; and a `Stream-Capable` capability bit
returned in a future `/capabilities` endpoint, so clients can
detect support without a probe request.

---

## 8. Calling Async Tools from Non-Async Clients

The Tokitai server is **fully async internally** — the request
handler is an `async fn`, the worker pool is a `tokio` runtime,
and tools that return `Future`s are awaited on the same
runtime. None of this leaks into the wire protocol: from the
client's perspective, `POST /call` is just an ordinary
synchronous HTTP request. There is no event loop, no `Promise`,
no `goroutine`, no `Future` to await on the client side.

**Concretely:** a sync Python `requests.post(...)` call works
fine for any tool, including tools whose Rust implementation
is `async fn`; a sync Go `http.Post(...)` works identically; a
blocking JavaScript script (top-level `await` is fine in
ES2022 modules) works too. The only thing that breaks this
model is **streaming** — the SSE response stays open for the
duration of the work, which does require an event-loop client.
The reference SDKs handle that for you; if you're rolling your
own, see `examples/curl/stream-tool.sh` for the
minimum-viable SSE consumer.

In short: **clients can be as simple as `curl` or `requests`
for the 95% case.** Reach for async I/O only when you need
streaming.

---

## See also

- `examples/py/tokitai_client.py` — Python SDK source.
- `examples/js/tokitai-client.ts` — TypeScript SDK source.
- `examples/go/tokitai.go` — Go SDK source.
- `examples/curl/{list,call,stream}-tool.sh` — curl helpers.
- `docs/MCP_ARCHITECTURE.md` — internal server wiring.
- `docs/AI_INTEGRATION.md` — feeding `GET /tools` output into
  an LLM planner.
