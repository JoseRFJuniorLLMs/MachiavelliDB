#!/usr/bin/env bash
# Native Linux/WSL gates. WSL's filesystem and /mnt/d must live on D:.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
export TMPDIR="${TMPDIR:-/mnt/d/tmp}"
mkdir -p "$TMPDIR"
report=$(mktemp -d "$TMPDIR/machiavelli-linux-verify-XXXXXXXX")
printf '%s\n' "$report" > "$TMPDIR/machiavelli-last-verification"
gate() {
  local name=$1; shift
  echo "Validating $name..."
  local start=$SECONDS code=0
  "$@" 2>&1 | tee "$report/$name.log" || code=$?
  printf '%s\t%s\t%s\n' "$name" "$code" "$((SECONDS-start))" >> "$report/results.tsv"
  [ "$code" -eq 0 ] || { echo "FAILED: $name; evidence: $report" >&2; exit "$code"; }
}
gate format bash scripts/rocks.sh fmt --all -- --check
gate tests bash scripts/rocks.sh test --workspace --features machiavelli-storage/rocksdb-backend --locked --offline
gate clippy bash scripts/rocks.sh clippy --workspace --all-targets --features machiavelli-storage/rocksdb-backend --locked --offline -- -D warnings
gate release bash scripts/rocks.sh build --release -p machiavelli-api -p machiavelli-cli --features machiavelli-storage/rocksdb-backend --locked --offline
gate smoke python3 scripts/smoke-api.py "$HOME/cargo-target/machiavelli/release"
# Throwaway Heraclitus build with the external-intent-v1 protocol; never the operator's service.
if [ -n "${MACHIAVELLI_E2E_HERACLITUS:-}" ]; then
  gate heraclitus python3 scripts/e2e-heraclitus.py "$HOME/cargo-target/machiavelli/release" "$MACHIAVELLI_E2E_HERACLITUS"
else
  echo "SKIPPED: heraclitus e2e (set MACHIAVELLI_E2E_HERACLITUS to a heraclitus-server binary)"
fi
if [[ " ${*:-} " == *" --qualification "* ]]; then
  gate cfr bash scripts/rocks.sh run --release -p machiavelli-cfr --example cfr-qualification --locked --offline
  # mmap on drvfs (/mnt/d) is ~100x slower; the WSL ext4 disk itself lives on D:\WSL.
  native_tmp="$HOME/tmp"
  mkdir -p "$native_tmp"
  gate arena env TMPDIR="$native_tmp" bash scripts/rocks.sh run --release -p machiavelli-cow-arena --example cow-arena-qualification --locked --offline
  grep -q "median_below_100us=true" "$report/arena.log" || { echo "FAILED: arena median >= 100us; evidence: $report" >&2; exit 1; }
fi
echo "PASS: native Linux gates; evidence: $report"
