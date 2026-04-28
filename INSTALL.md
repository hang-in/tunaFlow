# tunaFlow 설치 가이드

> This file is an AI-agent-readable install guide (used by Claude Code / Codex / Gemini during onboarding). **Human users**: [README.md](./README.md) has everything you need.

> 에이전트(Claude Code, Codex 등)가 직접 실행할 수 있도록 작성된 설치 가이드입니다.

## 전제 조건

### 1. 에이전트 CLI (필수 — 1개 이상)

```bash
# Claude Code (Anthropic)
npm install -g @anthropic-ai/claude-code

# Codex (OpenAI)
npm install -g @openai/codex

# Gemini (Google)
npm install -g @google/gemini-cli
```

### 2. 시스템 요구사항

- **macOS** 12 Monterey 이상 — Apple Silicon (arm64) 또는 Intel (x86_64)
- **Windows** 10 21H2 이상 — x64. WebView2 Runtime 자동 설치

### 3. Lite 트랙 선택 시 (권장)

Python 3 또는 Node.js 있으면 더 많은 기능 사용 가능:

```bash
# Python 3 확인
python3 --version || brew install python3

# Node.js 확인
node --version || brew install node
```

---

## 설치

### 방법 1: 스크립트 설치 (권장)

```bash
# Lite 트랙 (~20MB, 권장)
curl -fsSL https://raw.githubusercontent.com/hang-in/tunaFlow/main/install.sh | bash

# Full 트랙 (~250MB, 오프라인 환경)
curl -fsSL https://raw.githubusercontent.com/hang-in/tunaFlow/main/install.sh | bash -s -- --full
```

설치 스크립트가 자동으로 처리합니다:
- 최신 릴리즈 dmg 다운로드
- `/Applications/tunaFlow.app` 설치
- Gatekeeper 격리 속성 제거 (`xattr -cr`)
- `tunaflow` CLI 명령 등록 (`/usr/local/bin/tunaflow`)

### 방법 2: 수동 설치 (macOS)

1. [GitHub Releases](https://github.com/hang-in/tunaFlow/releases)에서 `.dmg` 다운로드
2. dmg 마운트 → tunaFlow.app을 /Applications로 복사
3. **Gatekeeper 우회 (필수)** — drag-install 만 한 경우 `.app` 에 quarantine
   속성이 부착되어 번들 안 sidecar (rawq) 실행이 차단됩니다 (exit 137,
   `Console.app` 에 "rejected"). 반드시 다음을 실행:
   ```bash
   xattr -cr /Applications/tunaFlow.app
   ```
   증상: 앱은 열리지만 footer 의 rawq 상태가 "unavailable" / "rawq sidecar
   없음" 으로 표시되거나, 코드 검색이 동작 안 함.
   설치 스크립트 (`install.sh`) 사용 시 자동 처리되므로 이 단계 불필요.

### 방법 3: Windows 설치

1. [GitHub Releases](https://github.com/hang-in/tunaFlow/releases)에서 `tunaFlow_*_x64-setup.exe` 다운로드
2. installer 실행
3. **SmartScreen 경고**: "Windows 의 PC 보호" 화면이 나오면 → **추가 정보** → **실행** 클릭
4. **백신 차단** (Windows Defender / Avast / 알약 등): 베타라 코드 서명이 없어 일부 백신이 false positive 로 차단할 수 있습니다.
   - 다운로드 차단된 경우 — 백신 격리함에서 복원 또는 다운로드 폴더 일시 예외 처리
   - 설치 후 실행 차단된 경우 — `C:\Users\{user}\AppData\Local\tunaFlow` 를 백신 예외 경로 추가
   - 안전 검증: VirusTotal 에 직접 업로드해서 false positive 패턴 확인 가능

NSIS installer 가 자동 처리:
- 설치 경로: `%LOCALAPPDATA%\tunaFlow`
- WebView2 Runtime 자동 다운로드 (없을 시)
- 시작 메뉴 단축키 추가
- 사용자 데이터: `%APPDATA%\tunaflow\`

---

## 실행

```bash
tunaflow
```

또는 Launchpad / Spotlight에서 "tunaFlow" 검색

---

## 코드 서명 우회 (베타 한정)

베타 단계라 코드 서명이 없습니다. OS 별 우회 방법:

### macOS Gatekeeper

"손상됐거나 열 수 없습니다" 메시지가 나오면:

```bash
xattr -cr /Applications/tunaFlow.app
```

### Windows SmartScreen

"Windows 의 PC 보호" 화면이 나오면 → **추가 정보** → **실행** 클릭.

### Windows 백신 차단 (Defender / Avast / 알약 등)

베타 unsigned NSIS installer 가 일부 백신의 false positive 트리거할 수 있습니다:
- 다운로드 차단 → 백신 격리함에서 복원
- 설치 후 차단 → `%LOCALAPPDATA%\tunaFlow` 를 예외 경로 추가
- 의심되면 [VirusTotal](https://www.virustotal.com) 에 직접 업로드 검증

향후 정식 release 시 Authenticode 코드 서명 도입 예정.

---

## Lite 트랙 — 추가 기능 자동 설치

앱 첫 실행 시 감지:

| 기능 | 필요 조건 | 없을 때 |
|------|----------|---------|
| code-review-graph | Python 3 + pip | 앱 내 안내 표시 |
| context-hub | Node.js + npm | 앱 내 안내 표시 |
| rawq | (번들 포함) | — |

---

## 문제 해결

```bash
# 앱이 안 열릴 때
xattr -cr /Applications/tunaFlow.app
open -a tunaFlow

# CLI 명령이 없을 때
/Applications/tunaFlow.app/Contents/MacOS/tunaFlow

# 로그 확인
open ~/Library/Logs/tunaFlow/
```

### rawq 가 인식 안 될 때 (footer "rawq sidecar 없음")

원인 별 분기:

| 증상 | 원인 | 해결 |
|------|------|------|
| 앱은 열리지만 rawq footer 가 "unavailable" / "rawq sidecar 없음" | macOS quarantine — drag-install 직후 `.app` 의 child 실행이 Gatekeeper 차단 | `xattr -cr /Applications/tunaFlow.app` 후 앱 재실행 |
| `cargo install rawq` 했는데도 인식 안 됨 | tunaFlow 는 시스템 PATH 의 rawq 가 아니라 앱 번들 내부 sidecar 만 사용 (의도된 디자인) | 시스템 rawq 영향 없음. 위의 quarantine 항목으로 분기 |
| 직접 빌드 시 `binaries/rawq-aarch64-apple-darwin doesn't exist` 로 중단 | `npm run tauri build` 직접 실행 시 사이드카 사전 빌드가 필요 | `./scripts/build.sh` (wrapper) 또는 빌드 전 `./scripts/build-rawq.sh` 실행 |
| 위 셋 모두 해결 후에도 unavailable | 코드측 sidecar resolution 실패 — `Console.app` 에서 `[get_rawq_status] rawq sidecar unavailable —` 로 시작하는 라인의 진단(triple, candidate count, PATH fallback) 확인 후 [Issue 등록](https://github.com/hang-in/tunaFlow/issues) | — |

### Smoke checklist (release 회귀 방지)

새 release DMG 검증 시 다음 4단계 모두 확인:

1. fresh DMG 다운로드 → /Applications drag → install.sh 우회 → 앱 실행 → Settings → Runtime 의 rawq 상태가 `ready` 또는 `built`
2. install.sh curl 경로 → 동일하게 `ready`
3. `./scripts/build.sh` 로 로컬 빌드 → /Applications 설치 → `ready`
4. (B 케이스) `cargo install rawq` 한 환경에서도 sidecar-only 동작 + footer 메시지가 안내성 있는지 확인

---

## 제거

```bash
rm -rf /Applications/tunaFlow.app
rm -f /usr/local/bin/tunaflow
```
