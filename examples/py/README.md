# Tokitai Python Client

A minimal async client for the [Tokitai](https://github.com/silverenternal/tokitai)
MCP-compatible HTTP server. Wraps `httpx` and exposes three operations that
mirror the Rust server's HTTP API.

## Install

```bash
# from a checkout of the tokitai repo
cd examples/py
pip install -e .

# or, copy tokitai_client.py into your project
```

The only runtime dependency is `httpx`. The `socks` extra pulls in
`httpx[socks]` for proxy support:

```bash
pip install "tokitai-client[socks]"
```

Requires Python 3.9+.

## Start the server

In a separate terminal, run the Rust example server that ships with the repo:

```bash
cargo run -p tokitai-mcp-server --example mcp_builder_demo
```

It binds to `http://127.0.0.1:8080` and exposes a `Calculator` with an
`add(a, b)` tool.

## Usage

```python
import asyncio
from tokitai_client import TokitaiClient


async def main() -> None:
    async with TokitaiClient("http://127.0.0.1:8080") as client:
        # 1. Discover tools
        tools = await client.list_tools()
        for tool in tools:
            print(f"- {tool.name}: {tool.description}")

        # 2. Call a tool
        resp = await client.call_tool("add", {"a": 1, "b": 2})
        if resp["success"]:
            print("result:", resp["result"])  # 3
        else:
            print("error:", resp["error"])

        # 3. Stream from an SSE-capable server (optional)
        try:
            async for chunk in client.stream_tool("add", {"a": 4, "b": 5}):
                print("chunk:", chunk)
        except NotImplementedError as e:
            print("SSE not available:", e)


asyncio.run(main())
```

## API summary

| Method | Description |
| --- | --- |
| `list_tools() -> list[McpTool]` | `GET /tools` |
| `call_tool(name, arguments) -> dict` | `POST /call` |
| `stream_tool(name, arguments) -> AsyncIterator[dict]` | `POST /sse/<name>` (if enabled) |

`stream_tool` raises `NotImplementedError` if the server returns 404 from
`/sse/<name>`, so the client fails fast on servers built without SSE.

## License

MIT OR Apache-2.0, same as Tokitai.
