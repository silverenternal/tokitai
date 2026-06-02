"""Tokitai Python SDK client.

A minimal async client for the Tokitai MCP-compatible HTTP server.

The server exposes three endpoints (see docs/CROSS_LANGUAGE.md):
    GET  /tools       -> list of McpTool definitions
    POST /call        -> invoke a tool, returns {success, result, error}
    GET  /sse/<name>  -> Server-Sent Events stream of tool output (optional)

Only the standard MCP-shaped subset is required. The SSE endpoint is
feature-gated: if the server does not expose it, ``stream_tool`` raises
a clear ``NotImplementedError`` rather than silently hanging.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, AsyncIterator, Mapping

import httpx


@dataclass
class McpTool:
    """Subset of the McpTool JSON shape that callers care about."""

    name: str
    description: str
    input_schema: dict[str, Any]

    @classmethod
    def from_json(cls, data: Mapping[str, Any]) -> "McpTool":
        return cls(
            name=data["name"],
            description=data.get("description", ""),
            input_schema=data.get("input_schema", {}),
        )


class TokitaiClient:
    """Async client for a Tokitai MCP server.

    Parameters
    ----------
    base_url:
        Origin of the server, e.g. ``"http://127.0.0.1:8080"``. Trailing
        slashes are tolerated.
    timeout:
        Default request timeout in seconds for unary calls. Streaming
        calls ignore this and use a long-lived connection.
    """

    def __init__(self, base_url: str, *, timeout: float = 30.0) -> None:
        self.base_url = base_url.rstrip("/")
        self._timeout = timeout
        # One shared client keeps connection pooling cheap.
        self._http = httpx.AsyncClient(base_url=self.base_url, timeout=timeout)

    async def __aenter__(self) -> "TokitaiClient":
        return self

    async def __aexit__(self, *exc: Any) -> None:
        await self.aclose()

    async def aclose(self) -> None:
        await self._http.aclose()

    # ------------------------------------------------------------------
    # Discovery
    # ------------------------------------------------------------------
    async def list_tools(self) -> list[McpTool]:
        """Return every tool the server has registered."""
        resp = await self._http.get("/tools")
        resp.raise_for_status()
        return [McpTool.from_json(item) for item in resp.json()]

    # ------------------------------------------------------------------
    # Unary invocation
    # ------------------------------------------------------------------
    async def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> dict[str, Any]:
        """Invoke ``name`` with ``arguments`` and return the raw response dict.

        The response always has the keys ``success`` (bool) and, on success,
        ``result`` (any JSON value) or, on failure, ``error`` (string). This
        method does not raise on tool-level errors — the caller is expected
        to inspect ``success``. HTTP-level errors (4xx/5xx) still raise.
        """
        payload = {"name": name, "arguments": dict(arguments or {})}
        resp = await self._http.post("/call", json=payload)
        resp.raise_for_status()
        return resp.json()

    # ------------------------------------------------------------------
    # Streaming (SSE)
    # ------------------------------------------------------------------
    async def stream_tool(
        self,
        name: str,
        arguments: Mapping[str, Any] | None = None,
    ) -> AsyncIterator[dict[str, Any]]:
        """Yield incremental results from the SSE endpoint.

        The server may not be built with SSE support. We probe ``/sse/<name>``
        with a HEAD request first and raise ``NotImplementedError`` if the
        endpoint is absent (404) so callers fail fast instead of waiting on
        a connection that will never deliver data.
        """
        # Probe for SSE support.
        probe = await self._http.head(f"/sse/{name}")
        if probe.status_code == httpx.codes.NOT_FOUND:
            raise NotImplementedError(
                f"Server {self.base_url!r} does not expose SSE streaming. "
                "Rebuild tokitai-mcp-server with the 'sse' feature or fall "
                "back to call_tool()."
            )

        url = f"/sse/{name}"
        # SSE uses long-lived chunked responses; disable the read timeout.
        async with self._http.stream(
            "POST", url, json={"name": name, "arguments": dict(arguments or {})}
        ) as resp:
            resp.raise_for_status()
            async for line in resp.aiter_lines():
                if not line or not line.startswith("data:"):
                    continue
                payload = line[len("data:"):].strip()
                if not payload:
                    continue
                try:
                    yield json.loads(payload)
                except json.JSONDecodeError:
                    yield {"raw": payload}


# ---------------------------------------------------------------------------
# Manual smoke test
# ---------------------------------------------------------------------------
async def _demo() -> None:  # pragma: no cover - illustrative
    """Print the call sequence for the canonical ``add(1, 2)`` example.

    This function is intentionally a no-network demo: it just shows the
    sequence a caller would issue. Run the Rust example server in another
    terminal first:

        cargo run -p tokitai-mcp-server --example mcp_builder_demo
    """
    print("Tokitai Python client demo")
    print("  1. GET  http://127.0.0.1:8080/tools")
    print("  2. POST http://127.0.0.1:8080/call  {name: 'add', arguments: {a: 1, b: 2}}")
    print("  -> expect {success: True, result: 3}")


if __name__ == "__main__":
    import asyncio
    asyncio.run(_demo())
