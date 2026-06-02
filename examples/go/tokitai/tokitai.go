// Package tokitai provides a minimal Go client for the Tokitai MCP-compatible
// HTTP server.
//
// The server exposes three endpoints (see docs/CROSS_LANGUAGE.md):
//
//	GET  /tools       -> []McpTool
//	POST /call        -> ToolCallResponse
//	POST /sse/<name>  -> text/event-stream (optional)
//
// Only the standard library is used.
package tokitai

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"
)

// McpTool mirrors the Rust McpTool JSON shape.
type McpTool struct {
	Name        string          `json:"name"`
	Description string          `json:"description"`
	InputSchema json.RawMessage `json:"input_schema"`
}

// ToolCallRequest is the body for POST /call.
type ToolCallRequest struct {
	Name      string         `json:"name"`
	Arguments map[string]any `json:"arguments"`
}

// ToolCallResponse is the body returned from POST /call.
type ToolCallResponse struct {
	Success bool            `json:"success"`
	Result  json.RawMessage `json:"result,omitempty"`
	Error   string          `json:"error,omitempty"`
}

// Client is a thread-safe, low-allocation HTTP client. It is safe to share
// across goroutines; each call takes its own context.
type Client struct {
	baseURL string
	http    *http.Client
}

// New returns a Client that talks to baseURL. Pass a fully qualified origin
// like "http://127.0.0.1:8080". The internal http.Client has a 30s timeout;
// use WithHTTPClient to override.
func New(baseURL string) *Client {
	return &Client{
		baseURL: strings.TrimRight(baseURL, "/"),
		http:    &http.Client{Timeout: 30 * time.Second},
	}
}

// WithHTTPClient replaces the underlying http.Client. Useful for tuning
// transports or for testing.
func (c *Client) WithHTTPClient(h *http.Client) *Client {
	c.http = h
	return c
}

// ListTools calls GET /tools.
func (c *Client) ListTools(ctx context.Context) ([]McpTool, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, c.baseURL+"/tools", nil)
	if err != nil {
		return nil, fmt.Errorf("build list_tools request: %w", err)
	}
	resp, err := c.http.Do(req)
	if err != nil {
		return nil, fmt.Errorf("list_tools: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode/100 != 2 {
		return nil, fmt.Errorf("list_tools: unexpected status %d", resp.StatusCode)
	}
	var tools []McpTool
	if err := json.NewDecoder(resp.Body).Decode(&tools); err != nil {
		return nil, fmt.Errorf("decode list_tools: %w", err)
	}
	return tools, nil
}

// CallTool calls POST /call.
func (c *Client) CallTool(ctx context.Context, name string, args map[string]any) (*ToolCallResponse, error) {
	if name == "" {
		return nil, errors.New("call_tool: name is required")
	}
	body, err := json.Marshal(ToolCallRequest{Name: name, Arguments: args})
	if err != nil {
		return nil, fmt.Errorf("marshal call_tool: %w", err)
	}
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, c.baseURL+"/call", bytes.NewReader(body))
	if err != nil {
		return nil, fmt.Errorf("build call_tool request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := c.http.Do(req)
	if err != nil {
		return nil, fmt.Errorf("call_tool: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode/100 != 2 {
		return nil, fmt.Errorf("call_tool: unexpected status %d", resp.StatusCode)
	}
	var out ToolCallResponse
	if err := json.NewDecoder(resp.Body).Decode(&out); err != nil {
		return nil, fmt.Errorf("decode call_tool: %w", err)
	}
	return &out, nil
}

// StreamEvent is a single Server-Sent Event.
type StreamEvent struct {
	Event string
	Data  string
	ID    string
}

// ErrSseNotSupported is returned by StreamCall when the server reports
// 404 on /sse/<name>. Callers can use errors.Is to detect it.
var ErrSseNotSupported = errors.New("server does not expose SSE streaming")

// StreamCall POSTs to /sse/<name> and yields parsed SSE events. The HTTP
// response body is read incrementally with a bufio.Scanner, so memory
// usage stays flat regardless of stream length. If the server returns
// 404, StreamCall returns ErrSseNotSupported and never starts reading.
//
// The caller's goroutine owns the loop; pass ctx to cancel mid-stream.
// StreamCall also accepts a non-SSE response and yields a single
// synthetic event containing the full body, so it works on servers that
// fall back to unary JSON without SSE framing.
func (c *Client) StreamCall(ctx context.Context, name string, args map[string]any) (<-chan StreamEvent, error) {
	body, err := json.Marshal(ToolCallRequest{Name: name, Arguments: args})
	if err != nil {
		return nil, fmt.Errorf("marshal stream_call: %w", err)
	}

	endpoint := c.baseURL + "/sse/" + url.PathEscape(name)
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, endpoint, bytes.NewReader(body))
	if err != nil {
		return nil, fmt.Errorf("build stream_call request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Accept", "text/event-stream")

	resp, err := c.http.Do(req)
	if err != nil {
		return nil, fmt.Errorf("stream_call: %w", err)
	}
	if resp.StatusCode == http.StatusNotFound {
		resp.Body.Close()
		return nil, ErrSseNotSupported
	}
	if resp.StatusCode/100 != 2 {
		resp.Body.Close()
		return nil, fmt.Errorf("stream_call: unexpected status %d", resp.StatusCode)
	}

	out := make(chan StreamEvent, 16)
	go func() {
		defer close(out)
		defer resp.Body.Close()

		// Non-SSE servers reply with a regular JSON body; emit it as one event.
		ct := resp.Header.Get("Content-Type")
		if !strings.HasPrefix(ct, "text/event-stream") {
			buf, _ := io.ReadAll(resp.Body)
			select {
			case out <- StreamEvent{Data: string(buf)}:
			case <-ctx.Done():
			}
			return
		}

		scanner := bufio.NewScanner(resp.Body)
		scanner.Buffer(make([]byte, 0, 64*1024), 1024*1024)
		var (
			dataLines []string
			eventName string
			id        string
		)
		flush := func() {
			if len(dataLines) == 0 && eventName == "" && id == "" {
				return
			}
			evt := StreamEvent{Event: eventName, Data: strings.Join(dataLines, "\n"), ID: id}
			dataLines = nil
			eventName = ""
			id = ""
			select {
			case out <- evt:
			case <-ctx.Done():
			}
		}
		for scanner.Scan() {
			line := scanner.Text()
			if line == "" {
				flush()
				continue
			}
			if strings.HasPrefix(line, ":") {
				continue
			}
			field, val, _ := strings.Cut(line, ":")
			if strings.HasPrefix(val, " ") {
				val = val[1:]
			}
			switch field {
			case "data":
				dataLines = append(dataLines, val)
			case "event":
				eventName = val
			case "id":
				id = val
			}
		}
		flush()
		_ = scanner.Err()
	}()

	return out, nil
}
