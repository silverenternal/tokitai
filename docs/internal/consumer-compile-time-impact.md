# Consumer-Facing Compile-Time Impact of `#[tool]`

**Date:** 2026-06-02
**Scope:** End-user-facing tooling that lets a downstream Tokitai
consumer measure the compile-time impact of `#[tool]` on **their
own crate**, not just on a fixture inside the tokitai repo.
**Author:** Documentation accompanying `scripts/measure-consumer-impact.sh`.

---

## 1. Why this exists

The internal `docs/internal/compile-time-optimization.md` and
`docs/internal/generated-code-size-audit.md` measure compile-time
and expansion size **inside the tokitai repo**, on a fixed 5- or
10-method fixture. Those numbers are useful for tracking the
generator's own cost, but they do not answer the question a
downstream user actually has:

> "If I add a `#[tool]` impl block to MY crate, how much slower
> does `cargo check` get on MY machine, and how much bigger is
> the expansion I am emitting?"

`scripts/measure-consumer-impact.sh` is the answer. It is a
zero-dependency shell script that:

1. Copies the user's crate into a temporary scratch directory
   (so the user's working tree is never modified).
2. Measures the **baseline** `cargo check` wall-clock time
   (median of 3 runs after a warmup).
3. Generates N synthetic `#[tool]` impl blocks (default N=5),
   each with M methods (default M=10), and injects them into
   the copy.
4. Measures the **augmented** `cargo check` wall-clock time
   (median of 3 runs after a warmup).
5. Reports the per-impl-block overhead in milliseconds and the
   expansion size of one synthetic `#[tool]` block.
6. Cleans up the scratch directory on exit (`trap` on `EXIT`).

## 2. Usage

```sh
# From the tokitai repo root (script lives at scripts/measure-consumer-impact.sh):
bash scripts/measure-consumer-impact.sh /path/to/your-crate

# With custom N, M, runs, warmup:
TOKITAI_N=10 TOKITAI_M=20 TOKITAI_RUNS=5 TOKITAI_WARMUP=2 \
    bash scripts/measure-consumer-impact.sh /path/to/your-crate
```

Configuration is via env vars (all optional):

| Env var          | Default | Meaning                              |
|------------------|--------:|--------------------------------------|
| `TOKITAI_N`      |       5 | Number of synthetic impl blocks      |
| `TOKITAI_M`      |      10 | Methods per synthetic block          |
| `TOKITAI_RUNS`   |       3 | Runs per measurement (median taken)  |
| `TOKITAI_WARMUP` |       1 | Discarded warmup runs                |
| `TOKITAI_PATH`   |  auto   | Path to the tokitai crate (auto-detected from the script's location) |
| `TOKITAI_QUIET`  |       0 | Set to 1 to suppress per-run output  |
| `TOKITAI_COLD`   |       1 | Set to 0 to *skip* `cargo clean` between runs (warm-cache mode) |

## 3. Expected output on the sample fixture

Running the script on the in-tree `sample-consumer-crate`
fixture (a blank-slate lib.rs with three plain functions) on
Linux / 8 threads, with `cargo-expand` installed, produces
(typical run, single representative output):

```
Tokitai compile-time impact measurement
  user crate:     /…/scripts/fixtures/sample-consumer-crate
  tokitai path:   /…/tokitai
  N (impl blocks): 5
  M (methods):     10
  runs:            3 (+ 1 warmup)
  scratch:         /tmp/tokitai-measure.XXXXXX

=== Baseline: cargo check on the unmodified user crate ===
  warmup 1...
  baseline run 1: 4.744913s
  baseline run 2: 4.843310s
  baseline run 3: 4.868037s

=== Injecting 5 synthetic #[tool] impl blocks (each with 10 methods) ===

=== Augmented: cargo check on the modified user crate ===
  warmup 1...
  augmented run 1: 4.902475s
  augmented run 2: 5.052790s
  augmented run 3: 4.830636s

=== Measuring expansion size via cargo expand ===

==========================================================
  Tokitai #[tool] compile-time impact report
==========================================================
  user crate:                 /…/scripts/fixtures/sample-consumer-crate
  N synthetic impl blocks:    5 (each with M=10 methods)
  runs per measurement:       3 (+ 1 warmup)

  baseline  cargo check median:  4.843310s
  augmented cargo check median:  4.902475s
  total delta:                   0.059s
  per #[tool] impl block:        11.8 ms
  per #[tool] method:            1.18 ms

  expansion size (one synthetic
   #[tool] block, 10 methods):    22897 bytes
  (full expansion saved to: /tmp/tokitai-expand.XXXXXX/expanded.rs)
==========================================================
```

The per-method cost (≈ 1-6 ms on this fixture) is a useful
order-of-magnitude figure for **small consumer crates with no
prior cross-crate type information**. On a larger consumer
crate the per-method cost is typically **higher** (5-10 seconds
for a 50-method block) because rustc has more cross-crate type
information to chase and the optimization / monomorphisation
pass has more code to chew on — see
`docs/internal/generated-code-size-audit.md` §2.5 for the
reasoning. To measure a larger crate, point the script at
your own crate and increase `TOKITAI_N` / `TOKITAI_M`.

The per-run spread on this fixture is high (we have seen the
per-impl-block median vary from 12 ms to 168 ms across runs on
the same machine) because the absolute time is dominated by
the tokitai-macros rebuild (≈ 4.5 s) and the tokitai-macros
build time is itself very noisy on a busy machine. The script
uses medians to dampen that noise; for a more stable number
increase `TOKITAI_RUNS` (e.g. 10) and `TOKITAI_WARMUP` (e.g. 3).

## 4. Companion: capturing macro expansion as `&str`

The script uses `cargo expand` to capture the rendered token
stream to a file. The user can do the same by hand:

```sh
# install once
cargo install cargo-expand

# capture the expansion of a specific test / bin
cargo expand --lib > expanded.rs

# count items of interest
grep -c 'fn __TOOL_DEF_'      expanded.rs   # one per method
grep -c 'fn __call_'          expanded.rs   # one per method (sync)
grep -c 'static DEF: '        expanded.rs   # one per method
grep -c 'pub fn call_tool'    expanded.rs   # exactly 1
grep -c 'pub fn call_tool_sync' expanded.rs # exactly 1
wc    -c expanded.rs          # total bytes — the "expansion size"
```

The internal harness uses the same pattern but captures the
output at compile time via a `const __BENCH_EXPANDED_OUTPUT: &str`
emitted by a hidden `#[proc_macro_derive]` (see
`tokitai-macros/src/lib.rs::__bench_expand_ten_methods` and
`tokitai-macros/benches/macro_expand_bench.rs`). That pattern
is not directly available to consumers, but `cargo expand` is.

## 5. Cross-references

* **`docs/internal/compile-time-optimization.md`** — the
  internal optimization pass that drove the per-method token
  count down. Section 2 has the baseline numbers (21 966 bytes
  for a 10-method all-sync impl block on the unoptimised
  generator); section 3 lists the changes; section 4 confirms
  the changes are byte-equivalent on the generated tokens.
* **`docs/internal/generated-code-size-audit.md`** — the
  read-only audit of the generator's emitted code. Section 2.4
  has the scaling table (5 / 10 / 50 / 100 methods → ~210 /
  410 / 2 050 / 4 050 expansion lines) and section 2.5 has the
  compile-time impact estimate (5-10 s for a 50-method block
  on a moderately-sized consumer crate).
* **`tokitai-macros/benches/macro_expand_bench.rs`** — the
  internal criterion benchmark. It measures *post-expansion*
  constant propagation (not expansion itself), but pins the
  output size to 21 966 bytes ± 5 %.
* **`tokitai-macros/tests/compile_time_optimization_test.rs`** —
  the regression test that enforces the ± 5 % tolerance on the
  expansion size; any optimization that changes the generated
  tokens (in a way the audit warned about) trips this test.

## 6. Caveats

The script is a **measurement tool**, not a benchmark. The
following pitfalls are well known — please read them before
quoting a number in a PR description or design doc.

* **Measures `cargo check`, not `cargo build` or `cargo test`.**
  `cargo build` includes codegen + linking, which is roughly
  30-50 % more wall-clock than `cargo check` on a typical crate
  with `#[tool]` impls. `cargo test` is a superset of `cargo
  build`. If you care about "full" compile time, use `cargo
  build`; if you care about IDE round-trip, `cargo check` is
  the right proxy.
* **Warm-cache timings only.** The script does **not** measure
  cold-cache compile time by default — it deliberately lets
  cargo's incremental cache accumulate across the warmup and
  timed runs. This models the "warm cache" scenario the doc is
  targeting ("how much does the IDE round-trip slow down on a
  re-save?"), which is what most consumers actually care about.
  Set `TOKITAI_COLD=1` to opt into a per-run `cargo clean` for
  a worst-case number. Note: even with `TOKITAI_COLD=1`, the
  tokitai-macros build itself is amortised — `cargo clean`
  clears the target dir but the tokitai-macros source state
  inside that dir is rebuilt from the same `.rlib` cache that
  rustc keeps globally.
* **Parallel codegen.** `cargo check` uses rustc's default
  codegen unit count and parallel crate compilation. The
  results are not stable across runs on a multi-core box;
  expect ± 10 % jitter on the median. The script reports
  medians (not means) to dampen this.
* **Link-time.** If the consumer crate enables LTO or has
  large `static DEF: LazyLock<ToolDefinition>` items, the
  per-method number quoted will be higher than the script
  reports (which is `cargo check`, no codegen, no link).
* **Workspace interaction.** The script copies the user's
  crate into a scratch directory but does not copy the
  surrounding workspace. If the user crate is part of a
  workspace with shared `target-dir` settings, the scratch
  build will lose that context.
* **The macro emits `::tokitai_core::...` paths.** The user
  crate must declare `tokitai-core` as a *direct* dependency
  (not just a transitive one via `tokitai`). The script adds
  it automatically if missing; the user is responsible for
  keeping the dependency once they adopt the macro for real.
* **Numbers are host-specific.** A 6 ms / method figure on an
  8-thread Linux box will be very different on a 2-thread
  macOS laptop, an M-series Mac, or a 64-thread build server.
  Quote host + thread count alongside any number.
* **Pre-existing tokitai-macros build state.** As of 2026-06-02
  the tokitai repo's working tree contains uncommitted
  modifications to `tokitai-macros/src/tool/schema/gen.rs` that
  introduce a compile error. The script will report
  `cargo check failed in baseline run (exit 101)` until those
  modifications are resolved. The companion `tokitai`
  umbrella crate still builds because its cached `.rlib`s
  pre-date the broken edit. See
  `docs/internal/generated-code-size-audit.md` §7.4.

## 7. Reproduction

```sh
# Run the script on the in-tree fixture
bash scripts/measure-consumer-impact.sh \
    scripts/fixtures/sample-consumer-crate/

# Or, on a user crate
bash scripts/measure-consumer-impact.sh /path/to/your-crate
```

The fixture crate is a real, buildable Rust crate — it
declares `tokitai` and `tokitai-core` as path dependencies,
contains three plain functions, and is verified by
`cargo test` before being checked in.
