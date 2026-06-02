//! `database_tool` — a real-world Tokitai example.
//!
//! Wires a SQLite database (via `sqlx`) into a `#[tool]`-decorated service
//! and exposes it over the Model Context Protocol (MCP) HTTP transport.
//!
//! Tools exposed:
//!
//! | Tool               | Description                                |
//! |--------------------|--------------------------------------------|
//! | `list_users`       | Paginated list of users                    |
//! | `get_user`         | Fetch a single user by id                  |
//! | `create_user`      | Insert a new user, return the new id       |
//! | `update_user_email`| Update a user's email, return rows touched |
//! | `delete_user`      | Delete a user by id, return rows deleted   |
//!
//! Run with: `cargo run --release -p database_tool`
//! Then:    `curl http://127.0.0.1:8080/tools`
//!          `curl -X POST http://127.0.0.1:8080/call -d '{"name":"list_users","arguments":{"limit":10}}'`

use std::{env, path::PathBuf, sync::Arc, time::Duration};

use serde::Serialize;
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};
use tokitai::{tool, ToolError, ToolProvider};
use tokitai_mcp_server::{McpServerBuilder, MultiToolProvider};

// =====================================================================
// Domain types
// =====================================================================

/// A user row, returned by read tools.
///
/// Returned as a normal `struct` so the JSON response is well-typed; the
/// `#[derive(Serialize)]` lets `serde_json::to_value` produce a real
/// `{ "id": 1, "name": "...", ... }` envelope that the LLM can parse.
#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub created_at: String,
}

// =====================================================================
// Error type that maps cleanly to MCP / ToolError
// =====================================================================

/// Application-level errors.
///
/// We deliberately keep the error type tiny and the conversion to
/// [`ToolError`] explicit: every variant maps to a single MCP error kind
/// so the LLM gets a precise signal (`not_found` vs `validation_error`
/// vs `internal_error`).
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("user #{0} not found")]
    NotFound(i64),

    #[error("validation: {0}")]
    Validation(String),

    #[error("database: {0}")]
    Database(#[from] sqlx::Error),

    #[error("internal: {0}")]
    Internal(String),
}

/// Run an async DB closure on the current Tokio runtime, safely from
/// within an async context (e.g. an axum handler running on a worker
/// thread). `Handle::block_on` panics if called directly on a worker
/// thread, so we first move off the worker via `block_in_place`.
fn run_db<F, T>(f: F) -> T
where
    F: std::future::Future<Output = T>,
{
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(f))
}

impl From<AppError> for ToolError {
    fn from(err: AppError) -> Self {
        match err {
            AppError::NotFound(id) => ToolError::not_found(format!("user #{id} not found")),
            AppError::Validation(msg) => ToolError::validation_error(msg),
            AppError::Database(e) => ToolError::internal_error(format!("database error: {e}")),
            AppError::Internal(msg) => ToolError::internal_error(msg),
        }
    }
}

// =====================================================================
// Service
// =====================================================================

/// Service object that holds the connection pool. Every method is a tool.
///
/// `#[tool]` generates, at compile time:
///   * the JSON Schema for the input (visible to the LLM),
///   * a dispatcher that parses JSON args into typed parameters,
///   * a `ToolCaller` implementation that maps app errors to `ToolError`,
///   * the static `tool_definitions()` metadata for the MCP server.
#[derive(Clone)]
pub struct UserService {
    pool: SqlitePool,
}

impl UserService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[tool]
impl UserService {
    /// List up to `limit` users, ordered by id.
    #[tool(
        desc = "List up to `limit` users from the database, ordered by id. Returns a JSON array.",
        example_limit = 10,
        min_limit = 1,
        max_limit = 100
    )]
    pub fn list_users(&self, limit: i32) -> Result<Vec<User>, AppError> {
        let pool = self.pool.clone();
        run_db(async move {
            let rows =
                sqlx::query("SELECT id, name, email, created_at FROM users ORDER BY id LIMIT ?")
                    .bind(limit as i64)
                    .fetch_all(&pool)
                    .await?;

            rows.into_iter()
                .map(|row| {
                    Ok(User {
                        id: row.try_get("id")?,
                        name: row.try_get("name")?,
                        email: row.try_get("email")?,
                        created_at: row.try_get("created_at")?,
                    })
                })
                .collect::<Result<Vec<_>, sqlx::Error>>()
                .map_err(Into::into)
        })
    }

    /// Fetch a single user by id.
    #[tool(
        desc = "Fetch a single user by id. Returns an error of kind `not_found` if the user does not exist.",
        min_id = 1
    )]
    pub fn get_user(&self, id: i32) -> Result<User, AppError> {
        let pool = self.pool.clone();
        run_db(async move {
            let row = sqlx::query("SELECT id, name, email, created_at FROM users WHERE id = ?")
                .bind(id as i64)
                .fetch_optional(&pool)
                .await?;

            match row {
                Some(row) => Ok(User {
                    id: row.try_get("id")?,
                    name: row.try_get("name")?,
                    email: row.try_get("email")?,
                    created_at: row.try_get("created_at")?,
                }),
                None => Err(AppError::NotFound(id as i64)),
            }
        })
    }

    /// Insert a new user. Returns the new id.
    #[tool(
        desc = "Insert a new user with a unique email. Returns the new user's id on success.",
        min_length_name = 1,
        max_length_name = 80,
        min_length_email = 3,
        max_length_email = 254,
        pattern_email = "@"
    )]
    pub fn create_user(&self, name: String, email: String) -> Result<i64, AppError> {
        // Defence-in-depth: even though `#[tool(pattern_email = "@")]` already
        // rejects emails without '@', we also normalise and double-check here.
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            return Err(AppError::Validation("name must not be empty".into()));
        }

        let pool = self.pool.clone();
        let email_for_msg = email.clone();
        run_db(async move {
            let result = sqlx::query("INSERT INTO users (name, email) VALUES (?, ?)")
                .bind(trimmed_name)
                .bind(&email)
                .execute(&pool)
                .await
                .map_err(|e| match e {
                    sqlx::Error::Database(db) if db.is_unique_violation() => {
                        AppError::Validation(format!("email '{email_for_msg}' is already taken"))
                    }
                    other => AppError::Database(other),
                })?;

            Ok(result.last_insert_rowid())
        })
    }

    /// Update a user's email. Returns the number of rows touched (0 or 1).
    #[tool(
        desc = "Update a user's email by id. Returns the number of rows touched (0 or 1).",
        min_id = 1,
        min_length_email = 3,
        max_length_email = 254,
        pattern_email = "@"
    )]
    pub fn update_user_email(&self, id: i32, email: String) -> Result<i64, AppError> {
        let pool = self.pool.clone();
        let email_for_msg = email.clone();
        run_db(async move {
            let result = sqlx::query("UPDATE users SET email = ? WHERE id = ?")
                .bind(&email)
                .bind(id as i64)
                .execute(&pool)
                .await
                .map_err(|e| match e {
                    sqlx::Error::Database(db) if db.is_unique_violation() => {
                        AppError::Validation(format!("email '{email_for_msg}' is already taken"))
                    }
                    other => AppError::Database(other),
                })?;

            if result.rows_affected() == 0 {
                return Err(AppError::NotFound(id as i64));
            }

            Ok(result.rows_affected() as i64)
        })
    }

    /// Delete a user. Returns the number of rows deleted (0 or 1).
    #[tool(
        desc = "Delete a user by id. Returns the number of rows deleted (0 or 1).",
        min_id = 1
    )]
    pub fn delete_user(&self, id: i32) -> Result<i64, AppError> {
        let pool = self.pool.clone();
        run_db(async move {
            let result = sqlx::query("DELETE FROM users WHERE id = ?")
                .bind(id as i64)
                .execute(&pool)
                .await?;

            if result.rows_affected() == 0 {
                return Err(AppError::NotFound(id as i64));
            }

            Ok(result.rows_affected() as i64)
        })
    }
}

// =====================================================================
// Schema bootstrap / seed
// =====================================================================

/// Resolve the on-disk path of the SQLite database.
///
/// Default: in-memory (so the example works out of the box without
/// any filesystem permissions). Override with the `DATABASE_URL` env
/// var, e.g. `sqlite:./data/users.db` for a file-backed database.
fn resolve_db_path() -> String {
    if let Ok(url) = env::var("DATABASE_URL") {
        return url;
    }

    // Make sure the data dir exists so users can opt in to a file
    // db by exporting `DATABASE_URL=sqlite:./data/users.db`.
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("data");
    let _ = std::fs::create_dir_all(&path);

    // In-memory keeps the example self-contained and runnable on
    // any system without needing write access to the crate dir.
    "sqlite::memory:".to_string()
}

/// Open a SQLite pool, run the schema, and seed a few rows.
async fn bootstrap_database() -> SqlitePool {
    let url = resolve_db_path();
    tracing::info!(%url, "connecting to sqlite");

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&url)
        .await
        .expect("failed to connect to sqlite");

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT    NOT NULL,
            email       TEXT    NOT NULL UNIQUE,
            created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("failed to create users table");

    // Seed only if the table is empty, so the example is idempotent.
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .expect("failed to count users");

    if count == 0 {
        tracing::info!("seeding users table");
        let seed = [
            ("Ada Lovelace", "ada@example.com"),
            ("Alan Turing", "alan@example.com"),
            ("Grace Hopper", "grace@example.com"),
            ("Linus Torvalds", "linus@example.com"),
        ];
        for (name, email) in seed {
            sqlx::query("INSERT INTO users (name, email) VALUES (?, ?)")
                .bind(name)
                .bind(email)
                .execute(&pool)
                .await
                .expect("failed to seed user");
        }
    } else {
        tracing::info!(count, "users table already populated, skipping seed");
    }

    pool
}

// =====================================================================
// Entrypoint
// =====================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // -- logging ---------------------------------------------------------
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,sqlx=warn,tokitai=info".into()),
        )
        .with_target(false)
        .init();

    // -- database --------------------------------------------------------
    let pool = bootstrap_database().await;
    let service = Arc::new(UserService::new(pool));

    // -- tool inventory (for the startup banner) ------------------------
    println!("=== Tokitai × SQLite example ===\n");
    println!("Loaded tools:");
    for t in UserService::tool_definitions() {
        println!("  - {} : {}", t.name, t.description);
    }
    println!();

    // -- sanity-check that we can actually call a tool from Rust ---------
    let demo = service
        .call_tool("list_users", &tokitai::json!({ "limit": 5 }))
        .map_err(|e| format!("demo list_users failed: {e}"))?;
    println!("[demo] list_users(5) -> {}", demo);

    // -- wire the service into the MCP server ---------------------------
    // MultiToolProvider is the recommended way when you might add more
    // tool groups later (e.g. an `OrderService`, a `MetricsService`, ...).
    let mut multi = MultiToolProvider::new();
    multi.add((*service).clone());

    let server = McpServerBuilder::with_tool(multi)
        .with_host("127.0.0.1")
        .with_port(8080)
        .with_cors(true)
        .with_tracing(true)
        .build();

    println!("MCP server listening on http://127.0.0.1:8080");
    println!("  GET  /tools  - list available tools");
    println!("  POST /call   - call a tool        (body: {{\"name\":..., \"arguments\":{{...}}}})");
    println!("  GET  /health - liveness check");
    println!("\nTry:");
    println!("  curl http://127.0.0.1:8080/tools");
    println!(
        "  curl -X POST http://127.0.0.1:8080/call -H 'Content-Type: application/json' \\\n       -d '{{\"name\":\"list_users\",\"arguments\":{{\"limit\":3}}}}'"
    );
    println!("\nPress Ctrl+C to stop.\n");

    server.run().await?;
    Ok(())
}
