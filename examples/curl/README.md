# Tokitai curl Examples

A handful of shell scripts that drive a Tokitai MCP server with nothing
more than `curl` and `jq`. Useful for debugging, smoke tests, and as
reference shapes for clients in other languages.

## Requirements

- `bash`
- `curl` (any version with `--data` / `-N`)
- `jq` (only for the JSON helpers below)

## Start the server

```bash
cargo run -p tokitai-mcp-server --example mcp_builder_demo
```

It binds to `http://127.0.0.1:8080` and exposes a `Calculator` and
`TextTools` group.

## Scripts

| Script | One-liner |
| --- | --- |
| `list-tools.sh` | `curl -fsS http://127.0.0.1:8080/tools \| jq` — pretty-print every tool. |
| `call-tool.sh` | `NAME=add ARGS_JSON='{"a":1,"b":2}' ./call-tool.sh` — POST `/call` and print the response. |
| `stream-tool.sh` | `NAME=add ARGS_JSON='{"a":1,"b":2}' ./stream-tool.sh` — POST `/sse/<name>` with `curl -N` for live SSE output. |

All scripts honour `BASE_URL` to point at a non-default server:

```bash
BASE_URL=http://localhost:9000 ./list-tools.sh
```

## Examples

```bash
# List tools
./list-tools.sh

# Call add(1, 2)
NAME=add ARGS_JSON='{"a":1,"b":2}' ./call-tool.sh

# Uppercase some text
NAME=to_uppercase ARGS_JSON='{"text":"hello"}' ./call-tool.sh

# Stream an SSE response
NAME=add ARGS_JSON='{"a":10,"b":32}' ./stream-tool.sh
```

## License

MIT OR Apache-2.0.
