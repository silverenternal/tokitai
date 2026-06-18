# T-006 — Provider envelope test fixtures

These files are **recorded request payloads**, not live API captures. They
represent what each provider's `tools` / `tools[].function.parameters` /
`input_schema` / `inputSchema` field expects on the wire:

- `openai_chat_completion_request.json` — OpenAI `/v1/chat/completions`
  request body. The provider expects each tool to live under
  `tools[].function.parameters` (not `tools[].parameters`).
- `anthropic_messages_request.json` — Anthropic `/v1/messages` request
  body. The provider expects `input_schema` (snake_case) at the top
  level of each tool, NOT nested under a `function` key.
- `mcp_tools_list_response.json` — MCP `tools/list` JSON-RPC response.
  The provider expects `inputSchema` (camelCase) on each tool.

The round-trip test in `tokitai/tests/provider_envelope_test.rs` builds
a `tokitai_core::ToolDefinition` from a known input schema, emits it
into each provider envelope via `to_openai_function`,
`to_anthropic_tool`, and `to_mcp_tool`, and asserts no field is dropped
on the way through.

Refresh these fixtures by re-recording from the provider's docs (OpenAI
<https://platform.openai.com/docs/guides/function-calling>, Anthropic
<https://docs.anthropic.com/en/docs/build-with-claude/tool-use>, MCP
<https://modelcontextprotocol.io/>). They are not pinned to a specific
provider version because they are envelope *shape* samples, not
provider-runtime samples.