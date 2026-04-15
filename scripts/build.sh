#!/usr/bin/env bash
#
# tunaFlow all-in-one build script (macOS)
#
# Checks dependencies, builds sidecar binaries (rawq / crg / context-hub),
# installs npm deps, and runs `tauri build`. Emits a readable progress
# trail so you can see every step.
#
# Usage:
#   ./scripts/build.sh                    # full build
#   ./scripts/build.sh --skip-sidecars    # skip sidecar rebuilds (use existing binaries)
#   ./scripts/build.sh --no-bundle        # pass --no-bundle to tauri (skip .dmg packaging)
#
# Env overrides (optional, autodetected if unset):
#   RAWQ_SRC=<path>
#   CRG_SRC=<path>
#   CHUB_SRC=<path>
#
set -euo pipefail

# ─── Colors / progress helpers ───────────────────────────────────────────────

if [[ -t 1 ]]; then
  C_RESET=$'\033[0m'; C_BOLD=$'\033[1m'; C_DIM=$'\033[2m'
  C_RED=$'\033[31m'; C_GRN=$'\033[32m'; C_YLW=$'\033[33m'; C_BLU=$'\033[34m'; C_CYN=$'\033[36m'
else
  C_RESET=; C_BOLD=; C_DIM=; C_RED=; C_GRN=; C_YLW=; C_BLU=; C_CYN=
fi

step()    { printf "\n${C_BOLD}${C_BLU}▶ %s${C_RESET}\n" "$*"; }
substep() { printf "  ${C_DIM}›${C_RESET} %s\n" "$*"; }
ok()      { printf "  ${C_GRN}✓${C_RESET} %s\n" "$*"; }
warn()    { printf "  ${C_YLW}!${C_RESET} %s\n" "$*"; }
fail()    { printf "  ${C_RED}✗${C_RESET} %s\n" "$*" >&2; }
die()     { fail "$*"; exit 1; }

run() {
  # Run a command with its output streamed live (so build progress is visible).
  substep "$*"
  "$@"
}

# ─── Args ────────────────────────────────────────────────────────────────────

SKIP_SIDECARS=0
NO_BUNDLE=0
for arg in "$@"; do
  case "$arg" in
    --skip-sidecars) SKIP_SIDECARS=1 ;;
    --no-bundle)     NO_BUNDLE=1 ;;
    -h|--help)
      awk 'NR==1{next} /^#!/{next} /^#/{sub(/^# ?/,""); print; next} {exit}' "$0"
      exit 0 ;;
    *) die "Unknown argument: $arg  (try --help)" ;;
  esac
done

# ─── Paths ───────────────────────────────────────────────────────────────────

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
BIN_DIR="$ROOT_DIR/src-tauri/binaries"

# ─── 1. Platform check ──────────────────────────────────────────────────────

step "1/6  Platform check"

UNAME_S="$(uname -s)"
if [[ "$UNAME_S" != "Darwin" ]]; then
  warn "Non-macOS platform ($UNAME_S). Tauri build targets macOS; results may differ."
else
  ok "macOS $(sw_vers -productVersion) ($(uname -m))"
fi

# ─── 2. Dependency check ────────────────────────────────────────────────────

step "2/6  Dependency check"

need() {
  # need <cmd> <install hint>
  if command -v "$1" >/dev/null 2>&1; then
    ok "$1  ($("$1" --version 2>&1 | head -1))"
  else
    fail "$1 not found"
    printf "    ${C_DIM}install: %s${C_RESET}\n" "$2"
    return 1
  fi
}

MISSING=0
need node "brew install node@22"        || MISSING=1
need npm  "comes with node"             || MISSING=1
need rustc "https://rustup.rs"          || MISSING=1
need cargo "https://rustup.rs"          || MISSING=1
if [[ "$UNAME_S" == "Darwin" ]]; then
  if xcode-select -p >/dev/null 2>&1; then
    ok "Xcode CLT ($(xcode-select -p))"
  else
    fail "Xcode Command Line Tools not installed"
    printf "    ${C_DIM}install: xcode-select --install${C_RESET}\n"
    MISSING=1
  fi
fi

# Node version hint (package.json wants >=22)
if command -v node >/dev/null 2>&1; then
  NODE_MAJOR="$(node -p 'process.versions.node.split(".")[0]' 2>/dev/null || echo 0)"
  if (( NODE_MAJOR < 22 )); then
    warn "Node major = $NODE_MAJOR; tunaFlow targets >=22. Upgrade recommended."
  fi
fi

[[ $MISSING -eq 0 ]] || die "Missing dependencies above. Install them and retry."

# ─── 3. Sidecar binaries ────────────────────────────────────────────────────

step "3/6  Sidecar binaries (rawq / crg / chub)"

if [[ $SKIP_SIDECARS -eq 1 ]]; then
  substep "--skip-sidecars set; using whatever is already in src-tauri/binaries/"
  ls -1 "$BIN_DIR" 2>/dev/null | grep -v README || warn "no sidecar binaries present; Tauri build will fail."
else
  TARGET_TRIPLE="$(rustc --print host-tuple)"
  substep "host target: $TARGET_TRIPLE"
  mkdir -p "$BIN_DIR"

  # ── 3a. rawq ── (script already exists; delegates)
  if [[ -f "$BIN_DIR/rawq-$TARGET_TRIPLE" ]]; then
    ok "rawq sidecar already present (skip). Remove src-tauri/binaries/rawq-$TARGET_TRIPLE to rebuild."
  elif [[ -x "$ROOT_DIR/scripts/build-rawq.sh" ]]; then
    substep "Building rawq sidecar via scripts/build-rawq.sh …"
    "$ROOT_DIR/scripts/build-rawq.sh"
    ok "rawq sidecar built"
  else
    warn "scripts/build-rawq.sh missing; rawq sidecar not built."
  fi

  # ── 3b. crg (code-review-graph) — Python + PyInstaller ──
  # (CI 레퍼런스: .github/workflows/build.yml의 build-crg job)
  build_crg() {
    local src="$1"
    local dest="$BIN_DIR/crg-$TARGET_TRIPLE"
    local venv="$ROOT_DIR/src-tauri/target/crg-venv"
    substep "Building crg from $src (Python/PyInstaller) …"

    # Python 3 확인
    if ! command -v python3 >/dev/null 2>&1; then
      fail "python3 not found; skip crg build (install Python 3.11+ or set CRG_SRC to skip)"
      return 1
    fi
    substep "crg: python3 $(python3 --version 2>&1)"

    # 격리된 venv 사용 (사용자 전역 site-packages 오염 방지)
    if [[ ! -d "$venv" ]]; then
      substep "crg: python3 -m venv $venv"
      python3 -m venv "$venv"
    fi
    # shellcheck disable=SC1091
    source "$venv/bin/activate"

    substep "crg: pip install . + pyinstaller"
    ( cd "$src" && pip install --quiet --upgrade pip && pip install --quiet . pyinstaller )

    # CI와 동일: code_review_graph 모듈의 진입점 파일(__main__.py 등)을 찾아 pyinstaller에 전달
    local entry
    entry=$(python3 -c "
import code_review_graph, os, sys
pkg = os.path.dirname(code_review_graph.__file__)
for f in ('__main__.py','cli.py','main.py'):
    p = os.path.join(pkg, f)
    if os.path.exists(p):
        print(p); sys.exit(0)
" 2>/dev/null || true)
    if [[ -z "$entry" ]]; then
      fail "crg: could not locate code_review_graph entry (__main__.py / cli.py / main.py)"
      deactivate || true
      return 1
    fi
    substep "crg: pyinstaller --onefile  entry=$entry"
    ( cd "$src" && pyinstaller --onefile \
        --name "crg-$TARGET_TRIPLE" \
        --distpath "$BIN_DIR" \
        "$entry" )

    deactivate || true
    if [[ -f "$dest" ]]; then
      chmod +x "$dest"
      ok "crg sidecar built: $dest"
    else
      fail "crg: PyInstaller did not produce $dest"
      return 1
    fi
  }
  if [[ -f "$BIN_DIR/crg-$TARGET_TRIPLE" ]]; then
    ok "crg sidecar already present (skip)."
  else
    CRG_CANDIDATES=(
      "${CRG_SRC:-}"
      "$ROOT_DIR/vendor/code-review-graph"
      "$ROOT_DIR/../_research/_util/code-review-graph"
    )
    CRG_PICKED=""
    for c in "${CRG_CANDIDATES[@]}"; do
      # Python source → pyproject.toml 기준으로 탐색
      [[ -n "$c" && -f "$c/pyproject.toml" ]] && { CRG_PICKED="$c"; break; }
    done
    if [[ -n "$CRG_PICKED" ]]; then
      build_crg "$CRG_PICKED" || warn "crg build failed (계속 진행)"
    else
      warn "crg source not found. Set CRG_SRC=<path> to a dir with pyproject.toml. Skipping."
    fi
  fi

  # ── 3c. context-hub (chub) — Node, pkg targets cli/ subdir ──
  # (CI 레퍼런스: npm install --prefix cli ; pkg cli --targets ... --output ...)
  build_chub() {
    local src="$1"          # repo root (has package.json + cli/)
    local cli_dir="$src/cli"
    local dest="$BIN_DIR/chub-$TARGET_TRIPLE"

    if [[ ! -d "$cli_dir" || ! -f "$cli_dir/package.json" ]]; then
      fail "chub: $cli_dir/package.json not found (repo layout changed?)"
      return 1
    fi

    substep "Building chub from $src (pkg cli/) …"
    if [[ ! -d "$cli_dir/node_modules" ]]; then
      substep "chub: npm install --prefix $cli_dir"
      npm install --prefix "$cli_dir" --no-audit --no-fund
    fi

    # pkg target triple mapping
    local pkg_target="node18-macos-arm64"
    if [[ "$TARGET_TRIPLE" == "x86_64-apple-darwin" ]]; then
      pkg_target="node18-macos-x64"
    fi
    substep "chub: npx pkg \"$cli_dir\" → $pkg_target"
    npx --yes pkg "$cli_dir" --targets "$pkg_target" --output "$dest"
    chmod +x "$dest"
    ok "chub sidecar built: $dest"
  }
  if [[ -f "$BIN_DIR/chub-$TARGET_TRIPLE" ]]; then
    ok "chub sidecar already present (skip)."
  else
    CHUB_CANDIDATES=(
      "${CHUB_SRC:-}"
      "$ROOT_DIR/vendor/context-hub"
      "$ROOT_DIR/../_research/_util/context-hub"
    )
    CHUB_PICKED=""
    for c in "${CHUB_CANDIDATES[@]}"; do
      # context-hub 레이아웃: 루트 package.json + cli/package.json
      [[ -n "$c" && -f "$c/cli/package.json" ]] && { CHUB_PICKED="$c"; break; }
    done
    if [[ -n "$CHUB_PICKED" ]]; then
      build_chub "$CHUB_PICKED" || warn "chub build failed (계속 진행)"
    else
      warn "chub source not found. Set CHUB_SRC=<path> to a dir with cli/package.json. Skipping."
    fi
  fi

  # Summary
  substep "installed sidecars:"
  ls -lh "$BIN_DIR" 2>/dev/null | awk 'NR>1 && $NF != "README.md" {print "    "$NF"  ("$5")"}'
fi

# ─── 4. Frontend deps ───────────────────────────────────────────────────────

step "4/6  Frontend dependencies (npm)"

if [[ -d node_modules ]]; then
  ok "node_modules present (skip install). Delete node_modules to force reinstall."
else
  run npm install --no-audit --no-fund
  ok "npm deps installed"
fi

# ─── 5. Tauri production build ──────────────────────────────────────────────

step "5/6  Tauri build (release — this may take 5~10 minutes on first run)"

TAURI_ARGS=()
[[ $NO_BUNDLE -eq 1 ]] && TAURI_ARGS+=(-- --no-bundle)

run npm run tauri build "${TAURI_ARGS[@]}"

# ─── 6. Verify output ───────────────────────────────────────────────────────

step "6/6  Build output"

BUNDLE_DIR="$ROOT_DIR/src-tauri/target/release/bundle"
APP_PATH="$BUNDLE_DIR/macos/tunaFlow.app"
DMG_PATH="$(ls -1t "$BUNDLE_DIR"/dmg/*.dmg 2>/dev/null | head -1 || true)"

if [[ -d "$APP_PATH" ]]; then
  ok "App bundle: $APP_PATH"
  substep "size: $(du -sh "$APP_PATH" | awk '{print $1}')"
else
  fail "App bundle not found at expected path: $APP_PATH"
fi

if [[ -n "$DMG_PATH" && -f "$DMG_PATH" ]]; then
  ok "DMG:        $DMG_PATH"
  substep "size: $(du -sh "$DMG_PATH" | awk '{print $1}')"
elif [[ $NO_BUNDLE -eq 1 ]]; then
  substep "DMG skipped (--no-bundle)"
else
  warn "DMG not found; bundle step may have been skipped."
fi

echo
printf "${C_BOLD}${C_GRN}Build complete.${C_RESET}\n"
echo "  To run:       open \"$APP_PATH\""
if [[ -n "$DMG_PATH" ]]; then
  echo "  To install:   open \"$DMG_PATH\"   (drag into /Applications, then:"
  echo "                xattr -cr /Applications/tunaFlow.app  # strip Gatekeeper quarantine)"
fi
