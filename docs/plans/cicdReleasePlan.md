---
title: CI/CD Release Pipeline Plan
status: planned
created_at: 2026-04-14
priority: P1
---

# CI/CD Release Pipeline

> GitHub Actions로 빌드 자동화 → GitHub Releases에 배포 → `install.sh` 한 줄로 설치

---

## 0. 배포 트랙

의존성 번들 방식에 따라 두 트랙으로 배포. 사용자가 선택.

| | **Lite** | **Full** |
|---|---|---|
| 앱 크기 | ~20MB | ~250MB |
| rawq | ✅ 번들 | ✅ 번들 |
| code-review-graph | 첫 실행 시 자동 설치 (`pip`) | ✅ 번들 (PyInstaller) |
| context-hub (chub) | 첫 실행 시 자동 설치 (`npm`) | ✅ 번들 (`pkg`) |
| 인터넷 필요 | 첫 실행 시 | 설치 후 불필요 |
| 대상 | Python/Node 이미 있는 개발자 | 클린 환경, 오프라인 사용 |

**Release asset 명명:**
```
tunaFlow-v0.1.0-lite-aarch64.dmg
tunaFlow-v0.1.0-full-aarch64.dmg
```

### Lite 트랙 — 자동 설치 흐름
```
앱 첫 실행
  → code-review-graph 없음 감지
      → Python 있으면: pip install code-review-graph (백그라운드)
      → Python 없으면: 앱 내 안내 ("brew install python3")
  → context-hub 없음 감지
      → Node 있으면: npm install -g @aisuite/chub (백그라운드)
      → Node 없으면: 앱 내 안내 ("brew install node")
  → 설치 완료 → 기능 활성화
```

### Full 트랙 — 번들 빌드
```
code-review-graph → PyInstaller → crg-{triple} 단일 바이너리
context-hub       → pkg        → chub-{triple} 단일 바이너리
rawq              → cargo build → rawq-{triple}
  ↓
모두 src-tauri/binaries/ 에 배치 → tauri build → 번들 포함
```

---

## 1. 현황

현재 CI (`.github/workflows/ci.yml`):
- check only — cargo check, cargo test, tsc, vite build
- rawq sidecar: placeholder 파일만 생성 (실제 빌드 없음)
- 릴리즈 workflow 없음
- macOS only (Windows 미지원)

---

## 2. 목표 구조

```
push tag v*.*.*
  ↓
build.yml 트리거
  ├─ macOS (aarch64 + x86_64) → .dmg + .app.tar.gz
  └─ Windows (x86_64) → .msi + .exe
  ↓
rawq sidecar 빌드 (별도 job, 각 플랫폼)
  ↓
GitHub Release 생성 + assets 업로드
  ↓
install.sh / install.ps1 에서 최신 릴리즈 다운로드
```

---

## 3. Workflow 설계

### 3.1 트리거

```yaml
on:
  push:
    tags:
      - 'v*.*.*'        # 릴리즈
  workflow_dispatch:    # 수동 실행 (베타 테스트용)
```

### 3.2 Job 구성

```
jobs:
  build-rawq          # rawq sidecar 빌드 (각 플랫폼)
  build-tauri         # Tauri 앱 빌드 (rawq 결과물 의존)
  create-release      # GitHub Release 생성 + 업로드
```

### 3.3 build-rawq

| 플랫폼 | runner | 산출물 |
|--------|--------|--------|
| macOS arm64 | macos-latest (M1) | `rawq-aarch64-apple-darwin` |
| macOS x64 | macos-13 | `rawq-x86_64-apple-darwin` |
| Windows x64 | windows-latest | `rawq-x86_64-pc-windows-msvc.exe` |

- rawq는 별도 Rust 프로젝트 — 소스 경로 확정 필요 (현재 `scripts/build-rawq.sh` 참조)
- 빌드 결과물을 artifact로 저장 → build-tauri job에서 다운로드

### 3.4 build-tauri

```yaml
strategy:
  matrix:
    include:
      - platform: macos-latest     # arm64
      - platform: macos-13         # x86_64
      - platform: windows-latest   # x86_64

steps:
  - uses: actions/checkout@v4
  - uses: actions/setup-node@v4 (node 22)
  - uses: dtolnay/rust-toolchain@stable
  - uses: Swatinem/rust-cache@v2

  - name: Download rawq sidecar
    uses: actions/download-artifact@v4
    # rawq-{arch} → src-tauri/binaries/

  - name: npm install
  - name: tauri build
    uses: tauri-apps/tauri-action@v0
    env:
      APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
      APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
      APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
      APPLE_ID: ${{ secrets.APPLE_ID }}
      APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
      APPLE_APP_SPECIFIC_PASSWORD: ${{ secrets.APPLE_APP_SPECIFIC_PASSWORD }}
```

### 3.5 코드 서명

| 플랫폼 | 방법 | 비용 |
|--------|------|------|
| macOS | Apple Developer Program + notarization | $99/년 |
| macOS (베타) | ad-hoc 서명 (`--sign-identity "-"`) + Gatekeeper 우회 안내 | 무료 |
| Windows | 코드 서명 인증서 (EV) | $200~500/년 |
| Windows (베타) | 서명 없음 + SmartScreen 경고 안내 | 무료 |

**베타 단계**: ad-hoc 서명으로 진행, 사용자에게 우회 방법 안내

### 3.6 버전 자동화

태그에서 버전 추출 → `tauri.conf.json`에 자동 주입:

```yaml
- name: Set version from tag
  run: |
    VERSION=${GITHUB_REF_NAME#v}
    # tauri-action이 --config로 version 주입 가능
```

---

## 4. 설치 스크립트

### 4.1 macOS (`install.sh`)

```bash
#!/bin/bash
# tunaFlow installer
# Usage: curl -fsSL https://raw.githubusercontent.com/hang-in/tunaFlow/main/install.sh | bash

set -e

REPO="hang-in/tunaFlow"
ARCH=$(uname -m)  # arm64 or x86_64

echo "tunaFlow 설치 중..."

# 최신 릴리즈 URL 조회
LATEST=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep "browser_download_url.*${ARCH}.*dmg" \
  | cut -d '"' -f 4)

# 다운로드 + 설치
curl -L "$LATEST" -o /tmp/tunaFlow.dmg
hdiutil attach /tmp/tunaFlow.dmg -quiet
cp -R /Volumes/tunaFlow/tunaFlow.app /Applications/
hdiutil detach /Volumes/tunaFlow -quiet

# PATH 등록 (tunaflow 명령)
BIN_PATH="/usr/local/bin/tunaflow"
cat > "$BIN_PATH" << 'EOF'
#!/bin/bash
open -a tunaFlow "$@"
EOF
chmod +x "$BIN_PATH"

echo ""
echo "✅ 설치 완료!"
echo "터미널에서 'tunaflow'를 입력하여 실행하세요."
```

### 4.2 에이전트용 INSTALL.md

```markdown
# tunaFlow 설치 가이드 (에이전트 실행용)

## 전제조건 확인
1. curl 확인: `curl --version`
2. 에이전트 CLI 중 하나 이상 필요:
   - claude: `npm install -g @anthropic-ai/claude-code`
   - codex: `npm install -g @openai/codex`
   - gemini: `npm install -g @google/gemini-cli`

## 설치
curl -fsSL https://raw.githubusercontent.com/hang-in/tunaFlow/main/install.sh | bash

## Gatekeeper 우회 (서명 없는 베타 버전)
xattr -cr /Applications/tunaFlow.app

## 실행
tunaflow
```

---

## 5. GitHub Secrets 필요 목록

| Secret | 용도 | 베타 단계 |
|--------|------|---------|
| `APPLE_CERTIFICATE` | macOS 서명 | 불필요 (ad-hoc) |
| `APPLE_SIGNING_IDENTITY` | macOS 서명 | 불필요 |
| `APPLE_ID` | notarization | 불필요 |
| `GITHUB_TOKEN` | Release 생성 | 자동 제공 |

---

## 6. 구현 순서

### Phase 1 — 빌드 가능 상태 (🤖 Claude)
- [ ] `tauri.conf.json` 아이콘 경로 연결
- [ ] rawq sidecar 빌드 자동화 (`build-rawq.sh` → GitHub Actions 연동)
- [ ] `tauri.conf.json` 버전 자동화 (태그에서 주입)

### Phase 2 — Release workflow (🤖 Claude)
- [ ] `.github/workflows/build.yml` 작성
- [ ] `install.sh` 작성
- [ ] `INSTALL.md` (에이전트용) 작성

### Phase 2 — 검증 (👤 사용자)
- [ ] 베타 태그 `v0.1.0-beta.1` 발행
- [ ] 샌드박스에서 빌드된 앱 실행 + 동작 확인

### Phase 3 — 정식 배포 (👤 사용자 결정)
- [ ] Apple Developer Program 가입 + 코드 서명
- [ ] Windows 빌드 (macOS 배포 후)
- [ ] 자동 업데이트 (`tauri-plugin-updater`) 연동

---

## 7. 관련 파일

| 파일 | 변경 내용 |
|------|----------|
| `.github/workflows/build.yml` | 신규 — 릴리즈 빌드 workflow |
| `src-tauri/tauri.conf.json` | 아이콘 경로 추가, 버전 자동화 |
| `install.sh` | 신규 — macOS 설치 스크립트 |
| `install.ps1` | 신규 — Windows 설치 스크립트 |
| `INSTALL.md` | 신규 — 에이전트용 설치 가이드 |
| `scripts/build-rawq.sh` | CI 환경 대응 수정 |
