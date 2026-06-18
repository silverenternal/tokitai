# Tokitai workspace Makefile
#
# Convenience targets for the most common maintenance tasks. CI runs the
# underlying `cargo` commands directly; this file exists so contributors can
# reproduce the same steps locally without memorizing flag combinations.
#
# The trybuild snapshot targets pin the rustc version used to (re)generate
# the snapshots in tokitai-macros/tests/ui/*.stderr. See
# scripts/audit-ui-snapshots.sh for the matching orphan-fixture audit.

# Pinned rustc version for trybuild snapshot generation. Bump this in lockstep
# with .github/workflows/ci.yml (RUSTC_VERSION env var) when the workspace
# re-blesses snapshots against a newer compiler.
RUSTC_VERSION ?= 1.96.0
RUSTUP_TOOLCHAIN ?= $(RUSTC_VERSION)

# Cargo command used for trybuild snapshot tests. The `RUSTC_BOOTSTRAP=1`
# shim is not required at the workspace root — trybuild reads the version
# from the `// rustc-version: X.Y.Z` header we now embed in every .stderr.
TRYBUILD := cargo test

# Default target: print what this Makefile offers.
.PHONY: help
help:
	@echo "Tokitai workspace targets:"
	@echo "  make check                Run fmt + clippy + test on the workspace"
	@echo "  make refresh-ui-snapshots Re-bless tokitai-macros/tests/ui/*.stderr"
	@echo "  make audit-ui-snapshots   Verify every .stderr has a matching .rs"
	@echo "  make fmt                  cargo fmt --all"
	@echo "  make clippy               cargo clippy --workspace --all-features --all-targets -- -D warnings"
	@echo "  make test                 cargo test --workspace --all-features"

.PHONY: fmt
fmt:
	cargo fmt --all

.PHONY: clippy
clippy:
	cargo clippy --workspace --all-features --all-targets -- -D warnings

.PHONY: test
test:
	cargo test --workspace --all-features

.PHONY: check
check: fmt clippy test

# Re-bless the trybuild UI snapshots. The RUSTC_VERSION sidecar file at
# tokitai-macros/tests/ui/RUSTC_VERSION is updated to match the rustc
# version that produced the new snapshots. CI's `ui-snapshot-audit` job
# will fail if the installed rustc drifts from this sidecar.
#
# Why a sidecar instead of a `// rustc-version:` header inside each
# .stderr? trybuild (>= 1.0) compares .stderr byte-for-byte against
# actual compiler output, so an inlined header would always show up
# as a diff. The sidecar is the diff-safe home for this metadata.
#
# Usage:
#   make refresh-ui-snapshots                 # uses RUSTC_VERSION (default 1.96.0)
#   make refresh-ui-snapshots RUSTC_VERSION=1.97.0
#   RUSTUP_TOOLCHAIN=1.97.0 make refresh-ui-snapshots
.PHONY: refresh-ui-snapshots
refresh-ui-snapshots:
	@echo "Re-blessing trybuild UI snapshots with rustc $(RUSTC_VERSION)..."
	RUSTUP_TOOLCHAIN=$(RUSTUP_TOOLCHAIN) TRYBUILD=overwrite \
		cargo test -p tokitai-macros --test ui_tests
	@echo "Updating RUSTC_VERSION sidecar..."
	@echo "$(RUSTC_VERSION)" > tokitai-macros/tests/ui/RUSTC_VERSION
	@echo "Done. Review the diff with: git diff tokitai-macros/tests/ui/"

# Verify that every tests/ui/*.stderr has a matching .rs and that the
# RUSTC_VERSION sidecar is present. This is the same audit the ci.yml
# `ui-snapshot-audit` job runs.
.PHONY: audit-ui-snapshots
audit-ui-snapshots:
	@bash scripts/audit-ui-snapshots.sh
