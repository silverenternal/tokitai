// Command list-tools connects to a Tokitai MCP server, lists the tools
// it exposes, and prints them as JSON.
//
// Usage:
//
//	go run ./cmd/list-tools                     # uses 127.0.0.1:8080
//	BASE_URL=http://localhost:9000 go run ./cmd/list-tools
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"time"

	"github.com/silverenternal/tokitai/examples/go/tokitai"
)

func main() {
	base := os.Getenv("BASE_URL")
	if base == "" {
		base = "http://127.0.0.1:8080"
	}

	client := tokitai.New(base)
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	tools, err := client.ListTools(ctx)
	if err != nil {
		fmt.Fprintf(os.Stderr, "list-tools: %v\n", err)
		os.Exit(1)
	}

	enc := json.NewEncoder(os.Stdout)
	enc.SetIndent("", "  ")
	if err := enc.Encode(tools); err != nil {
		fmt.Fprintf(os.Stderr, "encode: %v\n", err)
		os.Exit(1)
	}
}
