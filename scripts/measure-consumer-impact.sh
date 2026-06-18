#!/usr/bin/env bash
# measure-consumer-impact.sh
#
# Measure the per-impl-block compile-time overhead of the
# `#[tool]` proc-macro on a user-provided Rust crate.
#
# Usage:
#   bash scripts/measure-consumer-impact.sh /path/to/user-crate
#
# The script:
#   1. Copies the user's crate into a temporary scratch directory.
#   2. Builds a "baseline" `cargo check` (median of 3 runs after
#      a warmup).
#   3. Injects N synthetic `#[tool]` impl blocks (each with M
#      methods) into a copy of the crate.
#   4. Rebuilds "augmented" `cargo check` (median of 3 runs after
#      a warmup).
#   5. Computes the per-impl-block overhead in milliseconds and
#      reports it.
#   6. If `cargo expand` is on PATH, also reports the expansion
#      size of one synthetic `#[tool]` block (matching the
#      `__BENCH_EXPANDED_OUTPUT: &str` pattern from
#      `tokitai-macros/benches/macro_expand_bench.rs`).
#   7. Cleans up the scratch directory on exit (trap on EXIT).
#
# Configuration via env vars (all optional):
#   TOKITAI_N         number of synthetic `#[tool]` impl blocks
#                     to inject (default 5)
#   TOKITAI_M         number of methods per synthetic block
#                     (default 10)
#   TOKITAI_RUNS      runs per measurement (default 3)
#   TOKITAI_WARMUP    warmup runs before timing (default 1)
#   TOKITAI_PATH      absolute path to the tokitai crate to use
#                     when the user crate does not already depend
#                     on tokitai (default: parent of this script)
#   TOKITAI_QUIET     set to 1 to suppress per-run progress output
#   TOKITAI_PROFILE   T-011: when set, the macro emits
#                     `cargo:warning=impl <Type> -> <N> tools, ms=<us>`
#                     for each `#[tool]` impl block. The script
#                     reads these warnings instead of wall-clock
#                     `cargo check` when this var is also set in
#                     the environment that runs the script
#                     (i.e. `TOKITAI_PROFILE=1 bash scripts/...`).
#                     Per-impl timing is much less noisy than
#                     wall-clock because it isolates macro cost
#                     from link / codegen / toml parsing.
#
# Exit codes:
#   0   measurement completed (results may be reported as N/A)
#   1   cargo check failed during baseline OR augmentation;
#       the script continues to print the report with whatever
#       measurements it was able to collect
#   2   bad invocation
#   4   could not locate the tokitai crate

set -euo pipefail

# --- argument parsing ---------------------------------------------------------

if [ $# -lt 1 ]; then
    cat >&2 <<'USAGE'
usage: measure-consumer-impact.sh <path-to-user-crate>

Measures the per-impl-block compile-time overhead of the
Tokitai `#[tool]` macro on the given crate. See
docs/internal/consumer-compile-time-impact.md for caveats.
USAGE
    exit 2
fi

USER_CRATE=$(cd "$1" && pwd -P)

if [ ! -f "$USER_CRATE/Cargo.toml" ]; then
    echo "error: $USER_CRATE does not contain a Cargo.toml" >&2
    exit 2
fi

# --- configuration ------------------------------------------------------------

N=${TOKITAI_N:-5}
M=${TOKITAI_M:-10}
RUNS=${TOKITAI_RUNS:-3}
WARMUP=${TOKITAI_WARMUP:-1}
QUIET=${TOKITAI_QUIET:-0}

# Resolve the tokitai path. Order of precedence:
#   1. $TOKITAI_PATH (explicit override)
#   2. Sibling of this script (works for in-tree use)
#   3. Error out with a hint
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd -P)
TOKITAI_PATH=${TOKITAI_PATH:-}
if [ -z "$TOKITAI_PATH" ]; then
    if [ -f "$SCRIPT_DIR/../tokitai/Cargo.toml" ]; then
        TOKITAI_PATH=$(cd "$SCRIPT_DIR/../tokitai" && pwd -P)
    else
        echo "error: could not locate the tokitai crate." >&2
        echo "       set TOKITAI_PATH=/path/to/tokitai/tokitai and retry." >&2
        exit 4
    fi
fi
if [ ! -f "$TOKITAI_PATH/Cargo.toml" ]; then
    echo "error: TOKITAI_PATH=$TOKITAI_PATH does not contain Cargo.toml" >&2
    exit 4
fi

# --- scratch directory with trap cleanup -------------------------------------

SCRATCH=$(mktemp -d -t tokitai-measure.XXXXXX)
log() { [ "$QUIET" = "1" ] || echo "$@"; }

cleanup() {
    if [ -n "${SCRATCH:-}" ] && [ -d "${SCRATCH:-}" ]; then
        rm -rf "$SCRATCH"
    fi
}
trap cleanup EXIT
trap 'cleanup; exit 130' INT TERM

log "Tokitai compile-time impact measurement"
log "  user crate:     $USER_CRATE"
log "  tokitai path:   $TOKITAI_PATH"
log "  N (impl blocks): $N"
log "  M (methods):     $M"
log "  runs:            $RUNS (+ $WARMUP warmup)"
log "  scratch:         $SCRATCH"

# --- copy user crate into scratch -------------------------------------------

cp -R "$USER_CRATE/." "$SCRATCH/"
chmod -R u+w "$SCRATCH"

# Make sure tokitai is on the dependency list. If it is, leave the
# version/path the user chose alone; if not, add a path-based
# dependency so the augmented `cargo check` can resolve `tool`.
# The macro emits `::tokitai_core::...` paths into the consumer
# crate, so we also need `tokitai-core` as a *direct* dep, not
# just a transitive one.
USER_TOML="$SCRATCH/Cargo.toml"
TOKITAI_CORE_PATH="$TOKITAI_PATH/../tokitai-core"
if [ ! -d "$TOKITAI_CORE_PATH" ]; then
    TOKITAI_CORE_PATH=""
fi

if ! grep -qE '^[[:space:]]*tokitai[[:space:]]*=' "$USER_TOML"; then
    log "  (user crate has no \`tokitai\` dep — adding path-based dep)"
    {
        echo ""
        echo "# Added by scripts/measure-consumer-impact.sh — remove after measurement."
        if [ -n "$TOKITAI_CORE_PATH" ]; then
            echo "tokitai-core = { path = \"$TOKITAI_CORE_PATH\" }"
        else
            echo "# tokitai-core is required as a direct dep by the macro but"
            echo "# could not be located next to the tokitai crate."
        fi
        echo "tokitai = { path = \"$TOKITAI_PATH\" }"
    } >> "$USER_TOML"
fi
# If tokitai is present but tokitai-core is not, add tokitai-core
# (the macro emits absolute paths into it).
if grep -qE '^[[:space:]]*tokitai[[:space:]]*=' "$USER_TOML" && \
   ! grep -qE '^[[:space:]]*tokitai-core[[:space:]]*=' "$USER_TOML"; then
    log "  (user crate has no \`tokitai-core\` dep — adding path-based dep)"
    if [ -n "$TOKITAI_CORE_PATH" ]; then
        {
            echo ""
            echo "# Added by scripts/measure-consumer-impact.sh — remove after measurement."
            echo "tokitai-core = { path = \"$TOKITAI_CORE_PATH\" }"
        } >> "$USER_TOML"
    else
        log "  WARNING: could not locate tokitai-core; the macro will fail."
        log "           Set TOKITAI_PATH to the parent of the tokitai crate."
    fi
fi

# Make sure `use tokitai::tool;` is in scope in lib.rs (or main.rs)
# so the synthetic blocks compile. We only add it if neither file
# already has it; we never modify user-written code beyond appending
# a `use` statement.
ensure_tool_import() {
    local src="$1"
    if [ ! -f "$src" ]; then
        return
    fi
    if grep -qE 'use[[:space:]]+tokitai(::|;)' "$src"; then
        return
    fi
    # Prepend the `use` statement after the existing top-of-file
    # `//!` / `///` doc comments, if any. We use a python3 one-liner
    # to avoid platform-specific awk regex escape issues (macOS
    # BSD awk and Linux gawk disagree on whether `\!` is a valid
    # escape).
    local tmp
    tmp=$(mktemp)
    python3 - "$src" > "$tmp" <<'PYEOF' || { rm -f "$tmp"; return 1; }
import sys
src_path = sys.argv[1]
inserted = False
with open(src_path) as f:
    lines = f.readlines()
out = []
for line in lines:
    stripped = line.lstrip()
    if not inserted and (stripped.startswith("//!") or
                        stripped.startswith("///") or
                        stripped.startswith("//") or
                        stripped.startswith("/*") or
                        stripped.strip() == ""):
        out.append(line)
        continue
    if not inserted:
        out.append("use tokitai::tool;\n")
        out.append("\n")
        inserted = True
    out.append(line)
if not inserted:
    out.append("use tokitai::tool;\n")
print("".join(out), end="")
PYEOF
    mv "$tmp" "$src"
}

for candidate in "$SCRATCH/src/lib.rs" "$SCRATCH/src/main.rs"; do
    ensure_tool_import "$candidate"
done

# --- timing primitive --------------------------------------------------------
#
# We use python3 for sub-second precision (works on Linux + macOS).
# Falls back to perl (also pre-installed on both). If neither is
# available, fall back to `date +%s` (second precision only).

now_seconds() {
    if command -v python3 >/dev/null 2>&1; then
        python3 -c 'import time; print(time.time())'
    elif command -v perl >/dev/null 2>&1; then
        perl -e 'print time'
    else
        # shellcheck disable=SC2317
        date +%s
    fi
}

# --- cargo check wrapper -----------------------------------------------------

TARGET_DIR_BASE="$SCRATCH/_target"

# T-011: when TOKITAI_PROFILE is set, the macro emits per-impl
# timing as `cargo:warning=impl <Type> -> <N> tools, ms=<us>`
# lines. We grep those out of the cargo log and emit them on
# stdout, one per line, so the caller can pipe them to a JSON
# aggregator (CI captures the median per-impl number and fails
# if it regresses >20%).
parse_profile_warnings() {
    # $1: path to cargo log file
    # prints one `<TYPE> <MICROS>` tuple per line on stdout, in
    # the order they appeared in the log.
    local log_file=$1
    if [ ! -f "$log_file" ]; then
        return 0
    fi
    # The line format is documented in
    # tokitai-macros/src/tool/mod.rs and pinned by
    # tokitai-macros/tests/per_impl_profile_test.rs.
    grep -oE 'cargo:warning=impl [^ ]+ -> [0-9]+ tools, ms=[0-9]+' \
        "$log_file" \
    | sed -E 's|cargo:warning=impl ([^ ]+) -> ([0-9]+) tools, ms=([0-9]+)|\1 \3|'
}

run_cargo_check() {
    # $1: sub-target dir name (e.g. "baseline" or "augmented")
    # emits the elapsed time on stdout, returns cargo's exit code.
    #
    # By default we `cargo clean` between runs to get a stable,
    # reproducible measurement: the only delta between runs is
    # the change in the source code. This means the tokitai-
    # macros crate is rebuilt every run, which inflates the
    # absolute numbers but makes the *per-impl-block* delta
    # very crisp.
    #
    # Set TOKITAI_COLD=0 to disable the per-run `cargo clean`
    # and let cargo's incremental cache accumulate. The per-impl
    # number will then be much smaller (and noisier) because
    # cargo's own overhead (~30 ms / invocation) dominates.
    local sub=$1
    local target_dir="$TARGET_DIR_BASE/$sub"

    if [ "${TOKITAI_COLD:-1}" = "1" ]; then
        rm -rf "$target_dir"
    fi
    mkdir -p "$target_dir"

    local t_start t_end elapsed rc log_file
    t_start=$(now_seconds)
    log_file="$target_dir/cargo.log"
    # T-011: when TOKITAI_PROFILE is set in this script's
    # environment, forward it into the cargo sub-invocation so
    # the macro emits per-impl warnings. The build script
    # forwards the env var to rustc only when it is non-empty.
    local profile_env=""
    if [ -n "${TOKITAI_PROFILE:-}" ]; then
        profile_env="TOKITAI_PROFILE=$TOKITAI_PROFILE"
    fi
    ( cd "$SCRATCH" && \
      CARGO_TARGET_DIR="$target_dir" \
      env $profile_env \
      cargo check 1>"$log_file" 2>&1 ) && rc=0 || rc=$?
    t_end=$(now_seconds)

    if [ "$rc" -ne 0 ]; then
        echo "error: cargo check failed in $sub run (exit $rc)" >&2
        echo "       full cargo output is at: $log_file" >&2
        if [ -f "$log_file" ]; then
            # Print the last 30 lines of cargo output for context.
            tail -n 30 "$log_file" >&2
        fi
        return "$rc"
    fi

    # Compute elapsed in a way that tolerates either float or int
    # values from now_seconds(). Both are printed to stdout.
    elapsed=$(awk -v a="$t_start" -v b="$t_end" 'BEGIN { printf "%.6f", b - a }')
    echo "$elapsed"

    # T-011: when profiling is on, also stream the per-impl
    # measurements on a file descriptor that survives the
    # function return. The caller can pull them via
    # `parse_profile_warnings <log_file>` for richer reporting.
    if [ -n "${TOKITAI_PROFILE:-}" ]; then
        : # caller will read parse_profile_warnings on the log
    fi
}

# --- median helper -----------------------------------------------------------

median_of() {
    # $@: list of numbers (one per line, in any order)
    local sorted
    sorted=$(printf '%s\n' "$@" | sort -n)
    awk -v data="$sorted" '
        BEGIN {
            n = split(data, arr, "\n")
            if (n == 0) { print "0"; exit }
            mid = int(n / 2)
            if (n % 2 == 1) { print arr[mid + 1] }
            else             { printf "%.6f", (arr[mid] + arr[mid + 1]) / 2 }
        }
    '
}

# --- baseline measurement ----------------------------------------------------

log ""
log "=== Baseline: cargo check on the unmodified user crate ==="

for i in $(seq 1 "$WARMUP"); do
    log "  warmup $i..."
    if ! run_cargo_check baseline >/dev/null; then
        log ""
        log "ERROR: baseline cargo check failed during warmup $i."
        log "       See the cargo output above. The script will exit."
        exit 1
    fi
done

BASELINE_RUNS=()
for i in $(seq 1 "$RUNS"); do
    if ! t=$(run_cargo_check baseline); then
        log ""
        log "ERROR: baseline cargo check failed on run $i. See cargo output above."
        exit 1
    fi
    BASELINE_RUNS+=("$t")
    log "  baseline run $i: ${t}s"
done
BASELINE_MEDIAN=$(median_of "${BASELINE_RUNS[@]}")

# --- inject synthetic #[tool] impl blocks -----------------------------------

log ""
log "=== Injecting $N synthetic #[tool] impl blocks (each with $M methods) ==="

SYNTH_LIB="$SCRATCH/src/_synthetic_tools.rs"
{
    echo "// Auto-generated by scripts/measure-consumer-impact.sh."
    echo "// Do not edit — this file is deleted when the scratch dir is cleaned up."
    echo ""
    echo "use tokitai::tool;"
    echo ""
    for i in $(seq 1 "$N"); do
        echo "#[allow(dead_code)]"
        echo "#[derive(Default)]"
        echo "pub struct Synthetic${i};"
        echo ""
        echo "#[tool]"
        echo "impl Synthetic${i} {"
        for j in $(seq 1 "$M"); do
            cat <<METHOD
    /// Synthetic tool method \`method_${j}\` on \`Synthetic${i}\`.
    /// Inert body — the macro doesn't introspect the body, only the
    /// signature, doc comment, and parameter list.
    pub fn method_${j}(&self, x: i32) -> i32 { x + ${j} }
METHOD
        done
        echo "}"
        echo ""
    done
} > "$SYNTH_LIB"

# Wire the synthetic file into lib.rs (or main.rs) so it gets
# compiled. The wire is a single `mod _synthetic_tools;` line.
WIRED=0
for candidate in "$SCRATCH/src/lib.rs" "$SCRATCH/src/main.rs"; do
    if [ -f "$candidate" ] && [ "$WIRED" = "0" ]; then
        echo "" >> "$candidate"
        echo "// Added by scripts/measure-consumer-impact.sh." >> "$candidate"
        echo "#[allow(unused_imports, dead_code)]" >> "$candidate"
        echo "mod _synthetic_tools;" >> "$candidate"
        WIRED=1
    fi
done

if [ "$WIRED" = "0" ]; then
    # No lib.rs or main.rs in the user crate — create a minimal
    # lib.rs that re-exports the synthetic file. This is the
    # last-resort path for crates that are pure examples/binaries.
    mkdir -p "$SCRATCH/src"
    cat > "$SCRATCH/src/lib.rs" <<'LIB'
// Minimal stub created by scripts/measure-consumer-impact.sh
// for crates that do not ship a lib.rs.
#[allow(unused_imports, dead_code)]
mod _synthetic_tools;
LIB
fi

# --- augmented measurement ---------------------------------------------------

log ""
log "=== Augmented: cargo check on the modified user crate ==="

for i in $(seq 1 "$WARMUP"); do
    log "  warmup $i..."
    if ! run_cargo_check augmented >/dev/null; then
        log ""
        log "ERROR: augmented cargo check failed during warmup $i."
        log "       See the cargo output above. The script will exit."
        exit 1
    fi
done

AUGMENTED_RUNS=()
for i in $(seq 1 "$RUNS"); do
    if ! t=$(run_cargo_check augmented); then
        log ""
        log "ERROR: augmented cargo check failed on run $i. See cargo output above."
        exit 1
    fi
    AUGMENTED_RUNS+=("$t")
    log "  augmented run $i: ${t}s"
done
AUGMENTED_MEDIAN=$(median_of "${AUGMENTED_RUNS[@]}")

# --- compute & report --------------------------------------------------------

DELTA=$(awk -v a="$AUGMENTED_MEDIAN" -v b="$BASELINE_MEDIAN" \
    'BEGIN { printf "%.3f", a - b }')
PER_IMPL_MS=$(awk -v a="$AUGMENTED_MEDIAN" -v b="$BASELINE_MEDIAN" -v n="$N" \
    'BEGIN { if (n > 0) printf "%.1f", (a - b) * 1000.0 / n; else print "N/A" }')
PER_METHOD_MS=$(awk -v a="$AUGMENTED_MEDIAN" -v b="$BASELINE_MEDIAN" -v n="$N" -v m="$M" \
    'BEGIN { if (n > 0 && m > 0) printf "%.2f", (a - b) * 1000.0 / (n * m); else print "N/A" }')

# --- expansion size via cargo expand ----------------------------------------

EXPANSION_SIZE="N/A (cargo-expand not installed)"
EXPANSION_PATH=""
EXP_TMP=""

if command -v cargo-expand >/dev/null 2>&1; then
    log ""
    log "=== Measuring expansion size via cargo expand ==="
    EXP_TMP=$(mktemp -d -t tokitai-expand.XXXXXX)
    cat > "$EXP_TMP/Cargo.toml" <<TOML
[package]
name = "tokitai-expand-fixture"
version = "0.1.0"
edition = "2021"
publish = false
[workspace]

[dependencies]
tokitai = { path = "$TOKITAI_PATH" }
serde_json = "1.0"
TOML
    mkdir -p "$EXP_TMP/src"
    {
        echo "use tokitai::tool;"
        echo ""
        echo "pub struct Sample;"
        echo ""
        echo "#[tool]"
        echo "impl Sample {"
        for j in $(seq 1 "$M"); do
            echo "    /// Synthetic tool method \`method_${j}\` on \`Sample\`."
            echo "    pub fn method_${j}(&self, x: i32) -> i32 { x + ${j} }"
        done
        echo "}"
    } > "$EXP_TMP/src/lib.rs"

    EXPANDED="$EXP_TMP/expanded.rs"
    if ( cd "$EXP_TMP" && \
         cargo expand --lib 1>/dev/null 2>"$EXP_TMP/expand.err" \
         > "$EXPANDED" ); then
        EXPANSION_SIZE=$(wc -c < "$EXPANDED" | tr -d ' ')
        EXPANSION_PATH="$EXPANDED"
    else
        EXPANSION_SIZE="N/A (cargo expand failed — see $EXP_TMP/expand.err)"
    fi
    # Note: we deliberately do NOT rm -rf $EXP_TMP here — the
    # report below prints the full expansion path so the user can
    # inspect it. The trap on EXIT cleans it up.
else
    log ""
    log "(skip expansion-size measurement: cargo-expand not on PATH."
    log " install with \`cargo install cargo-expand\` to enable it.)"
fi

# --- final report ------------------------------------------------------------

# T-011: when the script was invoked with TOKITAI_PROFILE set, the
# macro emitted per-impl `cargo:warning=impl <Type> -> <N> tools,
# ms=<us>` lines into the cargo log. We surface those as the
# *primary* timing signal because it isolates macro cost from
# cargo / rustc / link noise. Wall-clock `cargo check` numbers
# are still printed below as a sanity check.
if [ -n "${TOKITAI_PROFILE:-}" ]; then
    PROFILE_BASELINE_LOG="$TARGET_DIR_BASE/baseline/cargo.log"
    PROFILE_AUGMENTED_LOG="$TARGET_DIR_BASE/augmented/cargo.log"

    log ""
    log "=========================================================="
    log "  Tokitai per-impl profile report (TOKITAI_PROFILE=$TOKITAI_PROFILE)"
    log "=========================================================="
    log "  baseline per-impl timings (cargo:warning=impl <Type> ... ms=<us>):"
    if [ -f "$PROFILE_BASELINE_LOG" ]; then
        parse_profile_warnings "$PROFILE_BASELINE_LOG" \
            | awk '{ printf "    %-40s %8s us\n", $1, $2 }' \
            | tee /dev/stderr \
            | awk '{print $2}' > "$SCRATCH/.baseline_profile_us.txt"
        BASELINE_PROFILE_MEDIAN=$(median_of $(cat "$SCRATCH/.baseline_profile_us.txt" 2>/dev/null))
        log "    -> baseline per-impl median: ${BASELINE_PROFILE_MEDIAN} us"
    else
        log "    (no baseline log found)"
        BASELINE_PROFILE_MEDIAN="N/A"
    fi
    log ""
    log "  augmented per-impl timings (cargo:warning=impl <Type> ... ms=<us>):"
    if [ -f "$PROFILE_AUGMENTED_LOG" ]; then
        parse_profile_warnings "$PROFILE_AUGMENTED_LOG" \
            | awk '{ printf "    %-40s %8s us\n", $1, $2 }' \
            | tee /dev/stderr \
            | awk '{print $2}' > "$SCRATCH/.augmented_profile_us.txt"
        AUGMENTED_PROFILE_MEDIAN=$(median_of $(cat "$SCRATCH/.augmented_profile_us.txt" 2>/dev/null))
        log "    -> augmented per-impl median: ${AUGMENTED_PROFILE_MEDIAN} us"
        if [ "$BASELINE_PROFILE_MEDIAN" != "N/A" ] && [ "$BASELINE_PROFILE_MEDIAN" != "0" ]; then
            PROFILE_PER_IMPL_US=$(awk -v a="$AUGMENTED_PROFILE_MEDIAN" -v b="$BASELINE_PROFILE_MEDIAN" -v n="$N" \
                'BEGIN { if (n > 0) printf "%.1f", (a - b) / n; else print "N/A" }')
            log "    -> per #[tool] impl block: ${PROFILE_PER_IMPL_US} us (median)"
        fi
    else
        log "    (no augmented log found)"
    fi
    log "=========================================================="
    log ""
    log "Caveats (profile mode):"
    log "  * Microsecond-resolution timings measured by the macro itself,"
    log "    isolated from cargo / rustc / link overhead."
    log "  * Numbers include only the codegen pipeline; `syn::parse`"
    log "    and quote-rendering time dominate in some impls."
    log "  * CI captures the per-impl median as an artifact and"
    log "    fails if median regresses >20% (see .github/workflows/ci.yml)."
    log ""
fi

log ""
log "=========================================================="
log "  Tokitai #[tool] compile-time impact report"
log "=========================================================="
log "  user crate:                 $USER_CRATE"
log "  N synthetic impl blocks:    $N (each with M=$M methods)"
log "  runs per measurement:       $RUNS (+ $WARMUP warmup)"
log ""
log "  baseline  cargo check median:  ${BASELINE_MEDIAN}s"
log "  augmented cargo check median:  ${AUGMENTED_MEDIAN}s"
log "  total delta:                   ${DELTA}s"
log "  per #[tool] impl block:        ${PER_IMPL_MS} ms"
log "  per #[tool] method:            ${PER_METHOD_MS} ms"
log ""
log "  expansion size (one synthetic"
log "   #[tool] block, $M methods):    ${EXPANSION_SIZE} bytes"
if [ -n "$EXPANSION_PATH" ]; then
    log "  (full expansion saved to: $EXPANSION_PATH)"
fi
log "=========================================================="
log ""
log "Caveats:"
log "  * Measures \`cargo check\`, not \`cargo build\` / \`cargo test\`."
log "  * Warm-cache timings. Cold-cache can be 5-10x higher."
log "  * Includes compile of tokitai / tokitai-macros on first run."
log "  * Parallel codegen: results may vary on multi-core machines."
log "  * See docs/internal/consumer-compile-time-impact.md for details."
log ""
log "Scratch directory: $SCRATCH (cleaned up on exit)"
