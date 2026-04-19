#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_TRIPLE="$(rustc --print host-tuple)"
DEST_DIR="$ROOT_DIR/src-tauri/binaries"
DEST_PATH="$DEST_DIR/rawq-$TARGET_TRIPLE"
TARGET_DIR="$ROOT_DIR/src-tauri/target/rawq-sidecar"

if [[ -n "${RAWQ_SRC:-}" ]]; then
  CANDIDATES=("$RAWQ_SRC")
else
  CANDIDATES=(
    "$ROOT_DIR/vendor/rawq"
    "$ROOT_DIR/../tunaDish/vendor/rawq"
    "$ROOT_DIR/../_research/_util/rawq"
  )
fi

RAWQ_SRC_DIR=""
for candidate in "${CANDIDATES[@]}"; do
  if [[ -f "$candidate/Cargo.toml" ]]; then
    RAWQ_SRC_DIR="$candidate"
    break
  fi
done

if [[ -z "$RAWQ_SRC_DIR" ]]; then
  echo "rawq source not found. Set RAWQ_SRC or place rawq at one of:" >&2
  printf '  %s\n' "${CANDIDATES[@]}" >&2
  exit 1
fi

echo "[rawq] source: $RAWQ_SRC_DIR"
echo "[rawq] target: $TARGET_TRIPLE"

mkdir -p "$DEST_DIR"

cargo build --manifest-path "$RAWQ_SRC_DIR/Cargo.toml" --release --target-dir "$TARGET_DIR"

cp "$TARGET_DIR/release/rawq" "$DEST_PATH"
chmod +x "$DEST_PATH"

echo "[rawq] installed sidecar: $DEST_PATH"
