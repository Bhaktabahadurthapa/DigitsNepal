#!/usr/bin/env bash
# Build the OpenWorker Harness runner that DigitsNepal Coworker bundles as a
# sidecar (Contents/MacOS/openworker-harness). Pinned to the same commit as
# the openworker-sdk dependency in src-tauri/Cargo.toml.
set -euo pipefail
REV="ecb445490fc054d52b8ea610d97ba947f57033d2"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
OUT="$ROOT/src-tauri/binaries/openworker-harness-$TRIPLE"
if [[ -x "$OUT" && "${1:-}" != "--force" ]]; then
  echo "Runner already built: $OUT"; exit 0
fi
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cargo install --locked --git https://github.com/andrewyng/openworker.git --rev "$REV" \
  openworker-sdk --bin openworker-harness --no-default-features --root "$TMP"
mkdir -p "$(dirname "$OUT")"
cp "$TMP/bin/openworker-harness" "$OUT"
echo "Built $OUT"
