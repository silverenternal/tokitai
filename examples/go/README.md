# Tokitai Go Client

A minimal Go client for the
[Tokitai](https://github.com/silverenternal/tokitai) MCP-compatible HTTP
server. Uses only the Go standard library.

## Layout

```
examples/go/
├── go.mod
├── tokitai/tokitai.go            # Client, types, StreamCall
├── cmd/list-tools/main.go        # CLI: print tools as JSON
└── README.md
```

## Install & build

```bash
cd examples/go
go build ./...
```

This compiles the library and the `list-tools` binary into the module
cache; nothing needs to be installed globally.

## Start the server

In a separate terminal:

```bash
cargo run -p tokitai-mcp-server --example mcp_builder_demo
```

It binds to `http://127.0.0.1:8080`.

## List tools

```bash
go run ./cmd/list-tools
```

Override the URL with `BASE_URL`:

```bash
BASE_URL=http://localhost:9000 go run ./cmd/list-tools
```

## Use the library

```go
package main

import (
	"context"
	"fmt"
	"log"

	"github.com/silverenternal/tokitai/examples/go/tokitai"
)

func main() {
	client := tokitai.New("http://127.0.0.1:8080")
	ctx := context.Background()

	tools, err := client.ListTools(ctx)
	if err != nil {
		log.Fatal(err)
	}
	for _, t := range tools {
		fmt.Printf("- %s: %s\n", t.Name, t.Description)
	}

	resp, err := client.CallTool(ctx, "add", map[string]any{"a": 1, "b": 2})
	if err != nil {
		log.Fatal(err)
	}
	fmt.Printf("add(1, 2): success=%v result=%s\n", resp.Success, string(resp.Result))
}
```

## Streaming (SSE)

```go
events, err := client.StreamCall(ctx, "add", map[string]any{"a": 1, "b": 2})
if errors.Is(err, tokitai.ErrSseNotSupported) {
    // server built without SSE — fall back to CallTool
}
for evt := range events {
    fmt.Println(evt.Data)
}
```

`StreamCall` posts to `/sse/<name>`, parses the response with
`bufio.Scanner`, and yields one `StreamEvent` per SSE record. Servers
that respond with non-SSE bodies emit a single synthetic event with the
full body, so the call still works as a graceful fallback.

## API

| Symbol | Description |
| --- | --- |
| `New(baseURL)` | Returns a `*Client` with a 30s timeout |
| `Client.ListTools(ctx)` | `GET /tools` |
| `Client.CallTool(ctx, name, args)` | `POST /call` |
| `Client.StreamCall(ctx, name, args)` | `POST /sse/<name>` |
| `ErrSseNotSupported` | Sentinel error from `StreamCall` on 404 |

## License

MIT OR Apache-2.0.
