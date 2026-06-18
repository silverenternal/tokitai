# MCP Spec Fixture — Pinned to 2025-06-18

This directory holds the minimal fixture data the `tokitai-mcp-server`
stdio transport is pinned against.

> **Re-sync note.** When the MCP spec revs, do the following:
>
> 1. Update `protocol-version.txt` to the new spec tag.
> 2. Edit `src/stdio.rs` and adjust:
>    - `MCP_PROTOCOL_VERSION`
>    - The list of `match` arms in `handle_request`
>    - Any new JSON-RPC error codes in `JsonRpcError`
> 3. Add a new sample request/response pair to `samples/` if the
>    spec adds new methods.
> 4. Re-run `cargo test -p tokitai-mcp-server --test mcp_stdio_smoke`
>    and bless any snapshot drift.
>
> We deliberately do **not** depend on `rmcp` or any other MCP SDK so
> the framer can be re-synced by hand against a moving spec without a
> moving dependency. See `docs/MCP_ARCHITECTURE.md` § "Stdio
> transport" for the full procedure.
