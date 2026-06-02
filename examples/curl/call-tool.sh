#!/usr/bin/env bash
# Invoke a single tool on a Tokitai MCP server.
#
# Usage:
#   NAME=add ARGS_JSON='{"a":1,"b":2}' ./call-tool.sh
#   NAME=to_uppercase ARGS_JSON='{"text":"hi"}' BASE_URL=http://localhost:9000 ./call-tool.sh
#
# Requirements: curl, jq
set -euo pipefail

NAME="${NAME:?NAME is required, e.g. NAME=add}"
ARGS_JSON="${ARGS_JSON:-{}}"
BASE_URL="${BASE_URL:-http://127.0.0.1:8080}"

payload=$(jq -nc --arg name "$NAME" --argjson args "$ARGS_JSON" \
    '{name: $name, arguments: $args}')

curl -fsS -X POST "${BASE_URL}/call" \
    -H "Content-Type: application/json" \
    -d "$payload" | jq
