#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_TRIPLE="$(rustc --print host-tuple)"
DEST_DIR="$ROOT_DIR/src-tauri/binaries"
DEST_PATH="$DEST_DIR/rawq-$TARGET_TRIPLE"
TARGET_DIR="$ROOT_DIR/src-tauri/target/rawq-sidecar"
RAWQ_REPO_URL="${RAWQ_REPO_URL:-https://github.com/hang-in/rawq}"

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

# Auto-clone fallback (last resort): if no local rawq source was found and
# RAWQ_SRC env was not explicitly set, clone the upstream repo into vendor/rawq.
# When RAWQ_SRC is set but invalid, do NOT auto-clone — surface the error so
# the user can fix their override path.
if [[ -z "$RAWQ_SRC_DIR" && -z "${RAWQ_SRC:-}" ]]; then
  AUTO_CLONE_DIR="$ROOT_DIR/vendor/rawq"
  if [[ -f "$AUTO_CLONE_DIR/Cargo.toml" ]]; then
    echo "[rawq] using existing auto-cloned vendor at $AUTO_CLONE_DIR"
    RAWQ_SRC_DIR="$AUTO_CLONE_DIR"
  else
    echo "[rawq] source not found locally — auto cloning $RAWQ_REPO_URL → $AUTO_CLONE_DIR"
    mkdir -p "$ROOT_DIR/vendor"
    if ! git clone --depth 1 "$RAWQ_REPO_URL" "$AUTO_CLONE_DIR"; then
      echo "[rawq] auto clone failed. Set RAWQ_SRC=/path/to/local/rawq or RAWQ_REPO_URL=<fork-url>." >&2
      echo "[rawq] searched candidates:" >&2
      printf '  %s\n' "${CANDIDATES[@]}" >&2
      exit 1
    fi
    RAWQ_SRC_DIR="$AUTO_CLONE_DIR"
  fi
fi

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
