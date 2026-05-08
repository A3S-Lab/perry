#!/usr/bin/env bash
# Run the Perry doc-example test harness.
#
# Compiles every .ts under docs/examples/, runs it (UI examples with
# PERRY_UI_TEST_MODE=1), and verifies compile + exit status + optional
# stdout diffs. Invoked on macOS/Linux CI; Windows uses run_doc_tests.ps1.

# `set -e` was the original setting, but the release_sweep.sh hook below
# needs to RUN AFTER cargo exits non-zero (because non-zero is the normal
# "some doc-tests failed" signal that we want to record in the summary
# JSON, not a hard abort that skips the summary entirely).
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$REPO_ROOT"

# Build perry + the harness in release mode (skipped if already built).
cargo build --release -p perry -p perry-runtime -p perry-stdlib -p perry-doc-tests

REPORT_DIR="$REPO_ROOT/docs/examples/_reports"
mkdir -p "$REPORT_DIR"

REPORT_JSON="$REPORT_DIR/latest.json"

# Forward any extra args through to the harness (e.g. --filter, --verbose).
# Was `exec cargo run ...` originally; now run as a subprocess so we can
# emit a flat summary for release_sweep.sh after the harness exits. The
# behavior of standalone runs is otherwise unchanged.
cargo run --release --quiet -p perry-doc-tests -- \
    --json "$REPORT_JSON" \
    "$@"
rc=$?

if [[ -n "${PERRY_TEST_SUMMARY_OUT:-}" ]] && [[ -f "$REPORT_JSON" ]]; then
    # Best-effort field extraction from the rust harness's pretty-printed
    # JSON. perry-doc-tests writes top-level "passed"/"failed"/"skipped"
    # fields (see crates/perry-doc-tests/src/main.rs).
    passed="$(sed -nE 's/.*"passed"[[:space:]]*:[[:space:]]*([0-9]+).*/\1/p' "$REPORT_JSON" | head -n1)"
    failed="$(sed -nE 's/.*"failed"[[:space:]]*:[[:space:]]*([0-9]+).*/\1/p' "$REPORT_JSON" | head -n1)"
    skipped="$(sed -nE 's/.*"skipped"[[:space:]]*:[[:space:]]*([0-9]+).*/\1/p' "$REPORT_JSON" | head -n1)"
    cat > "$PERRY_TEST_SUMMARY_OUT" <<EOF
{"script": "run_doc_tests.sh", "passed": ${passed:-0}, "failed": ${failed:-0}, "skipped": ${skipped:-0}, "exit_code": $rc}
EOF
fi

exit "$rc"
