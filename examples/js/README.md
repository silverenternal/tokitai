# Tokitai TypeScript Client

A zero-runtime-dependency TypeScript client for the
[Tokitai](https://github.com/silverenternal/tokitai) MCP-compatible HTTP
server. Uses only Web APIs (`fetch`, `EventSource`), so the same code
runs in Node 18+, browsers, Deno, and Bun.

## Install

```bash
cd examples/js
npm install
```

This pulls in `typescript`, `tsx`, and `@types/node` as devDependencies.
There are no runtime dependencies.

## Start the server

In a separate terminal:

```bash
cargo run -p tokitai-mcp-server --example mcp_builder_demo
```

It binds to `http://127.0.0.1:8080`.

## Run the example

```bash
# Type-check + emit
npm run build

# Run the example
npm start
```

Expected output:

```
Tokitai TypeScript client example
Server exposes N tool(s):
  - add: Add two numbers
  - ...
add(1, 2) = 3
```

## API

```ts
import { TokitaiClient } from "./tokitai-client.js";

const client = new TokitaiClient("http://127.0.0.1:8080");

await client.listTools();
const { success, result, error } = await client.callTool("add", { a: 1, b: 2 });

// Streaming (requires global EventSource):
for await (const evt of client.streamTool("add", { a: 1, b: 2 })) {
  console.log(evt.data);
}
```

| Method | Endpoint | Notes |
| --- | --- | --- |
| `listTools()` | `GET /tools` | Returns `McpTool[]` |
| `callTool(name, args)` | `POST /call` | Returns `ToolCallResponse` |
| `streamTool(name, args)` | `POST /sse/<name>` | Throws `SseNotSupportedError` on 404 |

## License

MIT OR Apache-2.0.
