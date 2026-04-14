#!/usr/bin/env bash
# tunaFlow installer
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/hang-in/tunaFlow/main/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/hang-in/tunaFlow/main/install.sh | bash -s -- --full

set -euo pipefail

REPO="hang-in/tunaFlow"
APP_NAME="tunaFlow"
INSTALL_DIR="/Applications"
APP_PATH="$INSTALL_DIR/${APP_NAME}.app"
BIN_PATH="/usr/local/bin/tunaflow"
TRACK="lite"

# ── Parse arguments ────────────────────────────────────────────────────────────
for arg in "$@"; do
  case "$arg" in
    --full) TRACK="full" ;;
    --lite) TRACK="lite" ;;
  esac
done

# ── Detect arch ───────────────────────────────────────────────────────────────
ARCH=$(uname -m)
case "$ARCH" in
  arm64)   TRIPLE="aarch64-apple-darwin" ;;
  x86_64)  TRIPLE="x86_64-apple-darwin" ;;
  *)       echo "지원하지 않는 아키텍처: $ARCH" >&2; exit 1 ;;
esac

echo "tunaFlow 설치 중... (트랙: $TRACK, 아키텍처: $ARCH)"

# ── Fetch latest release ───────────────────────────────────────────────────────
API_URL="https://api.github.com/repos/${REPO}/releases/latest"
if [[ "$TRACK" == "full" ]]; then
  ASSET_PATTERN="${TRIPLE}.*full.*\.dmg\|full.*${TRIPLE}.*\.dmg"
else
  ASSET_PATTERN="${TRIPLE}.*\.dmg"
fi

DMG_URL=$(curl -fsSL "$API_URL" \
  | grep "browser_download_url" \
  | grep -i "$TRIPLE" \
  | grep -i "\.dmg" \
  | grep -v "full" \
  | head -1 \
  | cut -d '"' -f 4)

if [[ "$TRACK" == "full" ]]; then
  DMG_URL=$(curl -fsSL "$API_URL" \
    | grep "browser_download_url" \
    | grep -i "$TRIPLE" \
    | grep -i "full" \
    | grep -i "\.dmg" \
    | head -1 \
    | cut -d '"' -f 4)
fi

if [[ -z "$DMG_URL" ]]; then
  echo "오류: $TRACK 트랙 ($ARCH) dmg를 찾을 수 없습니다." >&2
  echo "릴리즈 페이지를 직접 확인하세요: https://github.com/${REPO}/releases" >&2
  exit 1
fi

echo "다운로드 중: $DMG_URL"

# ── Download ───────────────────────────────────────────────────────────────────
TMP_DMG=$(mktemp /tmp/tunaflow_XXXXXX.dmg)
curl -L --progress-bar "$DMG_URL" -o "$TMP_DMG"

# ── Mount & install ────────────────────────────────────────────────────────────
MOUNT_POINT=$(mktemp -d /tmp/tunaflow_mnt_XXXXXX)
hdiutil attach "$TMP_DMG" -mountpoint "$MOUNT_POINT" -quiet

if [[ -d "$APP_PATH" ]]; then
  echo "기존 설치 제거 중..."
  rm -rf "$APP_PATH"
fi

cp -R "$MOUNT_POINT/${APP_NAME}.app" "$INSTALL_DIR/"

hdiutil detach "$MOUNT_POINT" -quiet 2>/dev/null || true
rm -f "$TMP_DMG"
rmdir "$MOUNT_POINT" 2>/dev/null || true

# ── Remove quarantine (ad-hoc signing, beta) ───────────────────────────────────
echo "Gatekeeper 격리 속성 제거 중..."
xattr -cr "$APP_PATH"

# ── CLI wrapper ────────────────────────────────────────────────────────────────
mkdir -p "$(dirname "$BIN_PATH")"
cat > "$BIN_PATH" << 'WRAPPER'
#!/usr/bin/env bash
open -a tunaFlow "$@"
WRAPPER
chmod +x "$BIN_PATH"

# ── Done ───────────────────────────────────────────────────────────────────────
echo ""
echo "설치 완료!"
echo ""
echo "실행 방법:"
echo "  tunaflow          # 터미널에서 실행"
echo "  open -a tunaFlow  # 또는 직접 실행"
echo ""
if [[ "$TRACK" == "lite" ]]; then
  echo "참고 (Lite 트랙):"
  echo "  code-review-graph, context-hub는 앱 첫 실행 시 자동 설치됩니다."
  echo "  Python 3 또는 Node.js가 없으면 일부 기능이 제한될 수 있습니다."
  echo ""
fi
echo "사전 준비: 에이전트 CLI 1개 이상 설치 필요"
echo "  npm install -g @anthropic-ai/claude-code"
echo "  npm install -g @openai/codex"
echo "  npm install -g @google/gemini-cli"
