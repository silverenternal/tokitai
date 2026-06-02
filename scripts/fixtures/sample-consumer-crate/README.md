# sample-consumer-crate

The **canonical blank-slate consumer crate** used by
`scripts/measure-consumer-impact.sh`.

It is intentionally small: a `lib.rs` with a handful of plain
functions and a single `tokitai` dependency. It contains **no
`#[tool]` impl blocks** — those are synthesised and injected by
the measurement script so the per-impl-block overhead is fully
attributable to the `#[tool]` macro.

## What "blank slate" means

* No `#[tool]`, `#[wrap]`, `#[openapi]`, `#[delegate]`, or
  resilience attributes anywhere in `src/`.
* No `tokitai::tool`, `tokitai::wrap`, etc. imports.
* The `Cargo.toml` declares `tokitai` only because the
  measurement script will inject synthetic `#[tool]` blocks at
  runtime; without the dependency the augmented `cargo check`
  would fail with a `use of undeclared crate` error.

## Usage

```sh
# From the tokitai repo root:
bash scripts/measure-consumer-impact.sh \
    scripts/fixtures/sample-consumer-crate/
```

The script copies this crate into a temporary scratch directory,
measures the baseline `cargo check` time, injects N synthetic
`#[tool]` impl blocks (default N=5, each with M=10 methods),
re-measures, and reports the per-impl-block overhead.

## Path-based dependency

The `Cargo.toml` pins `tokitai` to
`/home/hugo/codes/tokitai/tokitai/tokitai` so the fixture builds
out of the box from a clone of the repo. If you fork the repo
into a different absolute path, update the `path =` value in
`Cargo.toml` (or change it to `tokitai = "0.5"` to use a
published version).
