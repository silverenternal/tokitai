#!/usr/bin/env bash
# List the tools exposed by a Tokitai MCP server.
#
# Usage:
#   ./list-tools.sh                       # 127.0.0.1:8080/tools
#   BASE_URL=http://localhost:9000 ./list-tools.sh
#
# Requirements: curl, jq
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8080}"

curl -fsS "${BASE_URL}/tools" | jq
