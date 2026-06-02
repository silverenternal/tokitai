/**
 * Tokitai TypeScript client example.
 *
 * Calls the canonical `add(1, 2)` tool and prints the result. Run the
 * Rust example server in another terminal first:
 *
 *   cargo run -p tokitai-mcp-server --example mcp_builder_demo
 */

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

main().catch((err) => {
  console.error("fatal:", err);
  process.exit(1);
});
