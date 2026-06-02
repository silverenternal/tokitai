#!/usr/bin/env bash
# Stream a tool's output via Server-Sent Events.
#
# Usage:
#   NAME=add ARGS_JSON='{"a":1,"b":2}' ./stream-tool.sh
#   curl -N -X POST http://127.0.0.1:8080/sse/add \
#        -H 'content-type: application/json' \
#        -d '{"name":"add","arguments":{"a":1,"b":2}}'
#
# Requirements: curl. Disable buffering with -N so output streams live.
set -euo pipefail

NAME="${NAME:?NAME is required, e.g. NAME=add}"
ARGS_JSON="${ARGS_JSON:-{}}"
BASE_URL="${BASE_URL:-http://127.0.0.1:8080}"

payload=$(jq -nc --arg name "$NAME" --argjson args "$ARGS_JSON" \
    '{name: $name, arguments: $args}')

curl -N -fsS -X POST "${BASE_URL}/sse/${NAME}" \
    -H "Content-Type: application/json" \
    -H "Accept: text/event-stream" \
    -d "$payload"
