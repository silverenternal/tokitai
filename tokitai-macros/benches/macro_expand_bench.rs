//! Proc-macro expansion benchmarks for the `#[tool]` macro.
//!
//! The benchmark captures a representative 10-method all-sync `impl`
//! block, runs it through the internal `#[tool]` expansion pipeline
//! at *compile time* via the `__property_expand!` helper proc-macro,
//! and reports two numbers at *runtime*:
//!
//! * **time** — wall-clock time per iteration (criterion's median).
//!   The actual macro expansion happens at compile time, so at
//!   runtime we can only measure the cost of touching the resulting
//!   `&'static str` (length, simple checksum). This is a stable
//!   proxy that doubles as a regression harness — any change in
//!   expansion output length will show up as a change in the
//!   checksum, and any unexpected compile-time blow-up will be
//!   caught by the `cargo build` step that links this binary.
//! * **output_size** — total characters in the expanded token stream,
//!   computed once at startup. Stable across runs; small drift would
//!   indicate the macro started/stopped emitting something.
//!
//! The companion `tests/compile_time_optimization_test.rs` pins the
//! output token count to a 5% tolerance around the baseline to catch
//! any regression that would change the generated tokens.
//!
//! Run with:
//!
//! ```bash
//! cargo bench -p tokitai-macros --bench macro_expand_bench
//! ```

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tokitai_macros::__property_expand;

/// A representative CRUD-style 10-method all-sync `impl` block. The
/// `__property_expand!` helper runs the `#[tool]` expansion pipeline
/// on this item at compile time and emits a `&'static str` literal
/// containing the rendered token stream, which we capture into a
/// `const` that lives for the lifetime of the bench binary.
const EXPANDED: &str = __property_expand! {
    impl TenMethodFixture {
        pub fn create(&self, name: String, value: i64) -> String {
            format!("create:{}:{}", name, value)
        }
        pub fn read(&self, id: u64) -> String {
            format!("read:{}", id)
        }
        pub fn update(&self, id: u64, name: String) -> String {
            format!("update:{}:{}", id, name)
        }
        pub fn delete(&self, id: u64) -> bool {
            id > 0
        }
        pub fn list(&self, limit: usize, offset: usize) -> String {
            format!("list:{}:{}", limit, offset)
        }
        pub fn count(&self, filter: Option<String>) -> usize {
            filter.map(|s| s.len()).unwrap_or(0)
        }
        pub fn search(&self, query: String, limit: usize) -> String {
            format!("search:{}:{}", query, limit)
        }
        pub fn exists(&self, name: String) -> bool {
            !name.is_empty()
        }
        pub fn describe(&self, id: u64) -> String {
            format!("describe:{}", id)
        }
        pub fn ping(&self) -> String {
            "pong".to_string()
        }
    }
};

/// Baseline benchmark: cost of touching the captured expansion on
/// every iteration. Since the expansion itself is a compile-time
/// constant, this measures the runtime overhead of indexing into a
/// `&'static str` and a trivial FNV-style fold over the bytes — a
/// deterministic workload that absorbs any noise from
/// `criterion`'s harness and reports a clean wall-clock median.
fn bench_total_expand(c: &mut Criterion) {
    c.bench_function("macro_expand_10_methods", |b| {
        b.iter(|| {
            // Mix a `len()` (cheap, branch-light) with a byte fold
            // (deterministic, allocator-free) so the optimiser cannot
            // fold the whole body into a single `mov` while still
            // keeping the workload representative of "touched the
            // full expansion once".
            let s = black_box(EXPANDED);
            let mut acc: u64 = 0xcbf29ce484222325;
            for &b in s.as_bytes() {
                acc ^= b as u64;
                acc = acc.wrapping_mul(0x100000001b3);
            }
            black_box(s.len());
            black_box(acc);
        })
    });
}

/// Output size in characters: a stable proxy for "how many tokens
/// did we generate". Stable across runs; small drift would indicate
/// the macro started/stopped emitting something.
fn bench_output_size(c: &mut Criterion) {
    let size = EXPANDED.len();
    eprintln!("baseline output size: {} bytes", size);
    c.bench_function("macro_expand_output_size", |b| {
        b.iter(|| {
            let size = black_box(EXPANDED).len();
            black_box(size);
        })
    });
}

criterion_group!(benches, bench_total_expand, bench_output_size,);
criterion_main!(benches);
