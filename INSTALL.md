# tunaFlow 설치 가이드

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

- macOS 12 Monterey 이상
- Apple Silicon (arm64) 또는 Intel (x86_64)

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

### 방법 2: 수동 설치

1. [GitHub Releases](https://github.com/hang-in/tunaFlow/releases)에서 .dmg 다운로드
2. dmg 마운트 → tunaFlow.app을 /Applications로 복사
3. Gatekeeper 우회:
   ```bash
   xattr -cr /Applications/tunaFlow.app
   ```

---

## 실행

```bash
tunaflow
```

또는 Launchpad / Spotlight에서 "tunaFlow" 검색

---

## Gatekeeper 경고 문제

베타 단계에서는 Apple 코드 서명이 없습니다. "손상됐거나 열 수 없습니다" 메시지가 나오면:

```bash
xattr -cr /Applications/tunaFlow.app
```

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

---

## 제거

```bash
rm -rf /Applications/tunaFlow.app
rm -f /usr/local/bin/tunaflow
```
