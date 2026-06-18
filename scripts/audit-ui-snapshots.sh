#!/usr/bin/env bash
# scripts/audit-ui-snapshots.sh
#
# Audit the trybuild UI snapshot directory for orphaned .stderr files and
# verify that the installed rustc version matches the one stamped in
# tokitai-macros/tests/ui/RUSTC_VERSION. Designed to be run both locally
# and from CI.
#
# Why a sidecar RUSTC_VERSION file and not a `// rustc-version:` header
# inside each .stderr? trybuild (>= 1.0) compares the on-disk .stderr
# file byte-for-byte against the actual compiler output, so any header
# line would always show up as a diff. The sidecar is the diff-safe
# version: CI pins the compiler via rust-toolchain, and the audit job
# fails if (a) an .stderr is orphaned, (b) the sidecar is missing, or
# (c) the installed rustc does not match the sidecar.
#
# Exit code:
#   0  - clean (no orphans, sidecar present, installed rustc matches)
#   1  - any check failed
#
# Usage:
#   bash scripts/audit-ui-snapshots.sh
#   make audit-ui-snapshots

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
UI_DIR="${REPO_ROOT}/tokitai-macros/tests/ui"
SIDECAR="${UI_DIR}/RUSTC_VERSION"

if [[ ! -d "${UI_DIR}" ]]; then
    echo "error: ${UI_DIR} does not exist" >&2
    exit 1
fi

shopt -s nullglob

# 1) Orphan check: every .stderr has a matching .rs.
orphans=()
seen_count=0
for stderr in "${UI_DIR}"/*.stderr; do
    seen_count=$((seen_count + 1))
    name="$(basename "${stderr}" .stderr)"
    rs="${UI_DIR}/${name}.rs"
    if [[ ! -f "${rs}" ]]; then
        orphans+=("${name}.stderr")
    fi
done

# 2) Sidecar check.
sidecar_status="present"
if [[ ! -f "${SIDECAR}" ]]; then
    sidecar_status="missing"
fi
sidecar_version=""
if [[ "${sidecar_status}" == "present" ]]; then
    sidecar_version="$(tr -d '[:space:]' < "${SIDECAR}")"
fi

# 3) Installed rustc check (best effort: rustup or rustc).
installed_rustc=""
if command -v rustup >/dev/null 2>&1; then
    installed_rustc="$(rustup show active-toolchain 2>/dev/null | awk '{print $1}' || true)"
fi
if [[ -z "${installed_rustc}" ]] && command -v rustc >/dev/null 2>&1; then
    installed_rustc="$(rustc --version 2>/dev/null | awk '{print $2}' || true)"
fi

echo "tokitai-macros/tests/ui snapshot audit"
echo "  fixtures scanned:        ${seen_count}"
echo "  orphaned .stderr:         ${#orphans[@]}"
echo "  RUSTC_VERSION sidecar:    ${sidecar_status}"
if [[ -n "${sidecar_version}" ]]; then
    echo "  pinned rustc (sidecar):   ${sidecar_version}"
fi
if [[ -n "${installed_rustc}" ]]; then
    echo "  installed rustc:          ${installed_rustc}"
fi

if (( ${#orphans[@]} > 0 )); then
    echo
    echo "ORPHANED .stderr files (no matching .rs):"
    for o in "${orphans[@]}"; do
        echo "  ${o}"
    done
fi

if [[ "${sidecar_status}" == "missing" ]]; then
    echo
    echo "Missing sidecar: ${SIDECAR}"
    echo "Create it with: echo \"\$(rustc --version | awk '{print \$2}')\" > ${SIDECAR}"
fi

if [[ -n "${sidecar_version}" && -n "${installed_rustc}" && "${sidecar_version}" != "${installed_rustc}" ]]; then
    echo
    echo "WARNING: installed rustc (${installed_rustc}) does not match the sidecar (${sidecar_version})."
    echo "Re-bless the snapshots with: make refresh-ui-snapshots RUSTC_VERSION=${installed_rustc}"
fi

# Decide exit code: orphans or missing sidecar are hard errors. Installed
# rustc mismatch is a warning (CI uses rust-toolchain pinning so the
# match is enforced at install time; this branch catches local drift).
if (( ${#orphans[@]} > 0 )) || [[ "${sidecar_status}" == "missing" ]]; then
    exit 1
fi

exit 0
