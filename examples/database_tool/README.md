# `database_tool` — Tokitai × SQLite example

A self-contained, runnable example showing how to wire a real database
([`sqlx`](https://github.com/launchbadge/sqlx) + SQLite) into a Tokitai
service and expose it as an
[MCP](https://modelcontextprotocol.io/) HTTP server.

The example is intentionally small (one file, ~250 lines) but production-shaped:
it owns a real connection pool, runs a schema migration and a seed on
startup, and exposes five CRUD-style tools with parameter validation
and structured error mapping.

## What you get

Five tools, each with a JSON Schema an LLM can consume:

| Tool                 | Description                                     |
|----------------------|-------------------------------------------------|
| `list_users`         | Paginated list of users (`limit: 1..=100`)      |
| `get_user`           | Fetch one user by id; 404-style error if absent |
| `create_user`        | Insert a user with a unique email; returns id  |
| `update_user_email`  | Update one user's email; returns rows touched   |
| `delete_user`        | Delete a user by id; returns rows deleted       |

Each `#[tool(...)]` attribute emits a JSON Schema that includes the
`min`/`max`/`minLength`/`maxLength`/`pattern` constraints, so an LLM
client gets fast, deterministic feedback for bad input *before* the
function body ever runs.

## Run it

```bash
# from the workspace root
cargo run --release -p database_tool
```

The server binds to `http://127.0.0.1:8080`. By default it uses an
in-memory SQLite database (so it works on any machine with no file
permissions). To use a file-backed database instead, set:

```bash
DATABASE_URL=sqlite:./data/users.db cargo run --release -p database_tool
```

The `data/` directory is created on first run. The schema is migrated
and four seed rows are inserted if the table is empty, so the example
is idempotent across restarts.

## Talk to it

### 1. List tools (`GET /tools`)

```bash
$ curl -s http://127.0.0.1:8080/tools | jq '.[].name'
"list_users"
"get_user"
"create_user"
"update_user_email"
"delete_user"
```

### 2. Call a tool (`POST /call`)

```bash
$ curl -s -X POST http://127.0.0.1:8080/call \
    -H "Content-Type: application/json" \
    -d '{"name":"list_users","arguments":{"limit":2}}' | jq
```

```json
{
  "success": true,
  "result": [
    { "id": 1, "name": "Ada Lovelace", "email": "ada@example.com", "created_at": "2026-06-02 02:11:25" },
    { "id": 2, "name": "Alan Turing",  "email": "alan@example.com", "created_at": "2026-06-02 02:11:25" }
  ]
}
```

You can also use the generic shell clients under
[`examples/curl/`](../curl/):

```bash
NAME=list_users ARGS_JSON='{"limit":2}' ../curl/call-tool.sh
```

## Design notes

### Why `#[tool]` instead of `#[wrap]`

`#[tool]` is the right primitive here because we own the database and
the service code — we are not wrapping a third-party SDK. `#[tool]`
turns a regular `impl` block into a self-contained AI-callable surface
at compile time, with no extra layer between the LLM and the
function body. `#[wrap]` is for curating a subset of an existing
client API (e.g. a Stripe / OpenAI SDK); it would only add
indirection for a service we wrote from scratch.

`#[tool]` also gives us parameter-level validation (`min`, `max`,
`pattern`, `one_of`, `min_length`, `max_length`) that flows into
the JSON Schema the LLM sees, with zero runtime overhead: the
checks are emitted into the generated wrapper.

### How errors are mapped

The service has a tiny `AppError` enum:

```rust
enum AppError {
    NotFound(i64),
    Validation(String),
    Database(sqlx::Error),
    Internal(String),
}
```

…and an explicit `From<AppError> for ToolError`:

```rust
impl From<AppError> for ToolError {
    fn from(err: AppError) -> Self {
        match err {
            AppError::NotFound(id)        => ToolError::not_found(format!("user #{id} not found")),
            AppError::Validation(msg)     => ToolError::validation_error(msg),
            AppError::Database(e)         => ToolError::internal_error(format!("database error: {e}")),
            AppError::Internal(msg)       => ToolError::internal_error(msg),
        }
    }
}
```

We call the right `ToolError::X` constructor at every call site so
the **intent** is clear in the code, and so the message the LLM
sees is human-readable ("user #42 not found", not "DB row missing").

> **Caveat** — the current `#[tool]` macro hard-codes
> `ToolError::internal_error(format!("{}", e))` for the `Err` arm of
> a `Result<T, E>` return, so the *kind* in the wire response is
> always `InternalError` for application errors. The *message* still
> includes the original kind ("user #42 not found"), so the LLM can
> still react correctly. The full error pipeline is recoverable in
> a future macro release without changing user code.
>
> Parameter-level validation (the `min=`, `max=`, `pattern=` checks
> baked into the schema) **does** preserve `ValidationError` end to
> end, because it short-circuits before the function body runs.

### Sync tool bodies, async DB driver

`sqlx` is async-only, but `tokitai-mcp-server` invokes tool
implementations from a sync `ToolCaller::call_tool` entry point,
even though the surrounding axum handler is async. The macro
emits a sync-from-async bridge for `async fn` methods, but on a
multi-threaded Tokio runtime that bridge panics with "Cannot
start a runtime from within a runtime."

The clean fix is to keep the **tool surface** sync and let it call
the async DB code via `block_in_place` + `Handle::block_on`:

```rust
fn run_db<F, T>(f: F) -> T
where F: std::future::Future<Output = T>,
{
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(f))
}
```

Each tool becomes:

```rust
pub fn list_users(&self, limit: i32) -> Result<Vec<User>, AppError> {
    let pool = self.pool.clone();
    run_db(async move {
        sqlx::query("SELECT … FROM users ORDER BY id LIMIT ?")
            .bind(limit as i64)
            .fetch_all(&pool)
            .await
            .map_err(Into::into)
    })
}
```

This keeps the macro happy (no async bridge) and the runtime happy
(no nested runtime). It also gives us compile-time `#[tool]`
attribute parsing for `min`/`max`/`pattern` on the *parameters*,
even though the body is sync.

### What the LLM sees in the JSON Schema

`GET /tools` returns the schema. For `list_users`:

```json
{
  "name": "list_users",
  "description": "List up to `limit` users from the database, ordered by id. Returns a JSON array.",
  "input_schema": {
    "type": "object",
    "required": ["limit"],
    "properties": {
      "limit": {
        "type": "object",
        "description": "Maximum number of users to return (1-100)."
      }
    }
  }
}
```

`min`/`max` are emitted as runtime checks (the wrapper rejects
`limit < 1` before calling the function), but the current schema
generator only emits the parameter as a plain object because
primitive `i32` is not in the macro's known-types list. Adding
i32/i64/etc. to that list is on the tokitai roadmap; the
constraint still applies, just from the wrapper, not the schema.
`String` parameters do get the `pattern`/`minLength`/`maxLength`
constraints in the schema (see `create_user` below).

For `create_user`:

```json
{
  "name": "create_user",
  "description": "Insert a new user with a unique email. Returns the new user's id on success.",
  "input_schema": {
    "type": "object",
    "required": ["name", "email"],
    "properties": {
      "name":  { "type": "string", "minLength": 1, "maxLength": 80 },
      "email": { "type": "string", "minLength": 3, "maxLength": 254, "pattern": "@" }
    }
  }
}
```

The `pattern` is a substring check (`val.contains(pattern)`), not a
full regex. It is enough to catch "missing the @ symbol" at the
MCP boundary and avoid round-tripping an obviously bad request to
the database.

## Screenshots

Below are full request / response pairs from a live run, plus the
LLM's view of the input schema. (All output is real; nothing is
mocked.)

### Screenshot 1 — list users (success)

Request:

```bash
curl -s -X POST http://127.0.0.1:8080/call \
  -H 'Content-Type: application/json' \
  -d '{"name":"list_users","arguments":{"limit":3}}'
```

Response (pretty-printed):

```json
{
  "success": true,
  "result": [
    { "id": 1, "name": "Ada Lovelace",  "email": "ada@example.com",  "created_at": "2026-06-02 02:11:25" },
    { "id": 2, "name": "Alan Turing",   "email": "alan@example.com", "created_at": "2026-06-02 02:11:25" },
    { "id": 3, "name": "Grace Hopper",  "email": "grace@example.com", "created_at": "2026-06-02 02:11:25" }
  ]
}
```

LLM's view of the schema (from `GET /tools`):

```json
{
  "name": "list_users",
  "description": "List up to `limit` users from the database, ordered by id. Returns a JSON array.",
  "input_schema": {
    "type": "object",
    "required": ["limit"],
    "properties": {
      "limit": { "type": "object", "description": "Maximum number of users to return (1-100)." }
    }
  }
}
```

### Screenshot 2 — create user (success)

Request:

```bash
curl -s -X POST http://127.0.0.1:8080/call \
  -H 'Content-Type: application/json' \
  -d '{"name":"create_user","arguments":{"name":"Margaret Hamilton","email":"margaret@example.com"}}'
```

Response:

```json
{ "success": true, "result": 5 }
```

(`result: 5` is the new auto-increment id; this would be `5` on a
fresh DB, larger on subsequent runs.)

### Screenshot 3 — create user (validation: bad email)

This is the path where the macro's parameter-level validation
fires and the LLM gets a real `ValidationError` (kind and all):

Request:

```bash
curl -s -X POST http://127.0.0.1:8080/call \
  -H 'Content-Type: application/json' \
  -d '{"name":"create_user","arguments":{"name":"Test","email":"no-at-sign"}}'
```

Response:

```json
{
  "success": false,
  "error": "ToolError: ValidationError - parameter 'email' value 'no-at-sign' does not contain required pattern: @"
}
```

### Screenshot 4 — get user (not found)

Request:

```bash
curl -s -X POST http://127.0.0.1:8080/call \
  -H 'Content-Type: application/json' \
  -d '{"name":"get_user","arguments":{"id":99}}'
```

Response:

```json
{
  "success": false,
  "error": "ToolError: InternalError - user #99 not found"
}
```

The `kind` is `InternalError` (see the "How errors are mapped"
note above), but the message preserves "user #99 not found", so
the LLM can still branch on the substring.

### Screenshot 5 — list_users validation (below min)

Request:

```bash
curl -s -X POST http://127.0.0.1:8080/call \
  -H 'Content-Type: application/json' \
  -d '{"name":"list_users","arguments":{"limit":0}}'
```

Response:

```json
{
  "success": false,
  "error": "ToolError: ValidationError - parameter 'limit' value 0 is below the minimum 1"
}
```

## Project layout

```
examples/database_tool/
├── Cargo.toml
├── README.md
├── .gitignore
├── data/                  # created at runtime, ignored by git
└── src/
    └── main.rs            # the whole example
```

## Dependencies

The example's `Cargo.toml` adds the following crates (none of these
flow back into the Tokitai workspace itself):

| Crate                  | Why                                        |
|------------------------|--------------------------------------------|
| `sqlx`                 | Async SQLite driver + connection pooling    |
| `tokio`                | Async runtime                              |
| `serde` / `serde_json` | JSON ser/de for tool results and parameters |
| `tracing`              | Structured logging                         |
| `tracing-subscriber`   | `EnvFilter`-based logger init              |
| `thiserror`            | Ergonomic error enums                      |
| `tokitai-mcp-server`   | axum-based MCP HTTP server                 |
| `tokitai`              | `#[tool]` macro + re-exports               |
| `tokitai-core`         | Reachable so the macro's generated code compiles |

## License

MIT OR Apache-2.0, same as the rest of the Tokitai workspace.
