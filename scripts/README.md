# `scripts/` — Tokitai support scripts

End-user-facing tooling that lives alongside the Tokitai source
tree. None of these scripts are required to **use** Tokitai;
they exist to help downstream consumers measure, debug, and
inspect the macro.

| Script                              | Purpose                                                                                |
|-------------------------------------|----------------------------------------------------------------------------------------|
| `measure-consumer-impact.sh`        | Measure the per-impl-block compile-time overhead of `#[tool]` on a user crate.         |
| `fixtures/sample-consumer-crate/`   | The canonical "blank-slate" consumer crate used by `measure-consumer-impact.sh`.        |

## `measure-consumer-impact.sh`

Measures the wall-clock cost of `cargo check` on a user-provided
crate before and after injecting N synthetic `#[tool]` impl
blocks (each with M methods). Reports the per-impl-block overhead
in milliseconds and the expansion size of one synthetic block
(via `cargo expand`, if installed).

See:

* `docs/internal/consumer-compile-time-impact.md` — full guide
  on what the script measures, what it does NOT measure, and
  the known measurement pitfalls.
* `docs/internal/compile-time-optimization.md` — the internal
  optimization pass whose output sizes are the baseline for
  the "expansion size" report.
* `docs/internal/generated-code-size-audit.md` — the read-only
  audit that quantifies the per-impl-block overhead at the
  token level.

Quick start:

```sh
# From the tokitai repo root, on the in-tree fixture:
bash scripts/measure-consumer-impact.sh \
    scripts/fixtures/sample-consumer-crate/

# On your own crate:
bash scripts/measure-consumer-impact.sh /path/to/your-crate

# Custom N, M, runs, warmup (env vars):
TOKITAI_N=10 TOKITAI_M=20 TOKITAI_RUNS=5 \
    bash scripts/measure-consumer-impact.sh /path/to/your-crate
```

## `fixtures/sample-consumer-crate/`

A minimal buildable Rust crate that declares `tokitai` and
`tokitai-core` as path dependencies and exposes a handful of
plain functions. Contains **no** `#[tool]` impl blocks — those
are synthesised and injected by `measure-consumer-impact.sh`
so the per-impl-block overhead is fully attributable to the
macro itself.

The fixture is a real, buildable crate. `cargo test` in
`fixtures/sample-consumer-crate/` runs three unit tests and
exits 0. See `fixtures/sample-consumer-crate/README.md` for
details.

## Conventions

* All scripts use `#!/usr/bin/env bash` and `set -euo pipefail`.
* They work on both Linux and macOS, falling back gracefully
  when GNU-specific utilities are unavailable.
* They never modify the user's working tree — anything that
  needs to be measured is copied into a temporary scratch
  directory and cleaned up via `trap` on `EXIT`.
* They depend only on `cargo`, `bash`, `awk`, and (for the
  expansion-size report) `cargo expand`. No new Rust deps.
