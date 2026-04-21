<div align="center">

# tunaFlow

**AI Agent Orchestration Client**

[![Tauri 2](https://img.shields.io/badge/Tauri-2.0-FFC131?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![React 18](https://img.shields.io/badge/React-18-61DAFB?logo=react&logoColor=white)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-stable-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![DB Schema](https://img.shields.io/badge/DB_Schema-v30-8b5cf6)](.)
[![License](https://img.shields.io/badge/License-Private-9ca3af)](.)

[![Language: Korean](https://img.shields.io/badge/Language-한국어-2563eb)](./README.md)
[![English](https://img.shields.io/badge/English-9ca3af)](./README.en.md)

> **Of the agent, By the agent, For the agent**

도메인 전문가가 여러 AI 에이전트를 하나의 작업 흐름 안에서 운영하기 위한 데스크톱 클라이언트

</div>

![tunaFlow screenshot](./docs/assets/screenshot-main.png)

---

## 누구를 위한 도구인가

- Claude Code, Codex, Gemini CLI를 쓰면서 **대화 이상의 작업 구조**가 필요한 사람
- 에이전트에게 실행을 맡기되 **방향과 판단은 직접 유지**하고 싶은 사람
- AI 에이전트를 일상적인 개발 워크플로우에 넣으려는 소규모 팀 또는 개인

---

## 주요 기능

### Orchestration Workflow

Architect → Developer → Reviewer 3-role 시스템.
Plan을 설계하면 Developer가 구현하고, Reviewer가 교차 검증합니다.
실패 시 findings를 분석해 rev.N+1 Plan을 자동 제안합니다.

### Quick / Deep Review

- **Quick**: 단일 Reviewer가 빠르게 검증
- **Deep**: 복수 엔진이 Roundtable로 교차 검증 + 테스트 자동 주입. 5차원 루브릭(Plan Coverage · Code Quality · Test Coverage · Doc Quality · Convention) 기반 평가

### PTY Terminal

`-p` 플래그 없이 CLI 에이전트와 인터랙티브 세션을 실행합니다. 파일 수정, 명령 실행 등 에이전트의 전체 도구 사용이 가능합니다.

### Roundtable (RT)

여러 엔진의 에이전트가 하나의 주제로 토론합니다. Sequential(순차) 또는 Deliberative(동시) 모드. 모든 RT는 Branch의 확장입니다.

### ContextPack

4개 엔진 공통 프롬프트 조립 엔진. Lite / Standard / Full 자동 Tiering. rawq 코드 검색, 장기기억, 실패 학습, 역할 문서를 맥락에 포함합니다.

### Insight

rawq + code-review-graph가 사전 추출한 데이터를 에이전트에게 분석시킵니다. 안정성 · 테스트 · 아키텍처 · 성능 · 보안 · 기술부채 6개 카테고리. Quick Wins 자동 수정 지원.

### 메타 에이전트 온보딩

첫 프로젝트 설정 시 사용 가능한 에이전트를 자동 감지하고, 프로젝트 스택에 맞는 에이전트 구성을 추천합니다.

---

## 지원 엔진

| 엔진 | 연동 방식 |
|------|----------|
| Claude (Anthropic) | CLI subprocess |
| Codex (OpenAI) | CLI subprocess |
| Gemini (Google) | CLI subprocess |
| Ollama / LM Studio / vLLM | HTTP SSE |

---

## 설치 및 실행

### 사전 준비

- macOS (현재 macOS 전용)
- Node.js 20+, Rust stable
- 에이전트 CLI 1개 이상:

```bash
npm install -g @anthropic-ai/claude-code   # Claude
npm install -g @openai/codex               # Codex
npm install -g @google/gemini-cli          # Gemini
```

### 개발 실행

```bash
git clone https://github.com/hang-in/tunaFlow.git
cd tunaFlow
npm install
npm run tauri dev
```

### 빌드

```bash
./scripts/build.sh
```

### 베타 설치 (macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/hang-in/tunaFlow/main/install.sh | bash
```

> macOS ad-hoc 서명이므로 Gatekeeper 경고가 뜰 수 있습니다.
> `xattr -cr /Applications/tunaFlow.app` 으로 해제합니다.

---

## 기술 스택

Tauri 2 + React 18 + TypeScript + Zustand 5 + Tailwind CSS 4 + Rust + SQLite (WAL, v30)

코드 검색: rawq sidecar (bge-m3 임베딩) · code-review-graph · context-hub
외부 연동: HTTP API + WebSocket · MCP 서버 (`tunaflow-mcp`)

---

## 문서

| 문서 | 내용 |
|------|------|
| [CLAUDE.md](./CLAUDE.md) | 아키텍처, 컨벤션, 핸드오프 |
| [Architecture Detail](./docs/reference/architecture-detail.md) | RT 흐름, Store 구조, DB 스키마 |
| [Implementation Status](./docs/reference/implementationStatus.md) | 기능별 구현 현황 |
| [Beta Release Plan](./docs/plans/betaReleaseReadinessPlan.md) | 배포 준비 체크리스트 |
| [Dev History](./docs/reference/devHistory.md) | 프로젝트 계보 + 개발 이력 |

---

## 알려진 제약 (Beta)

- **macOS 전용** — Windows/Linux 빌드는 후속 과제
- **ad-hoc 서명** — Gatekeeper 경고 해제 필요 (`xattr -cr /Applications/tunaFlow.app`)
- **RT 중간 스트리밍 미지원** — Roundtable 은 라운드 단위로만 결과 표시
- **대규모 인덱싱 시 CPU 스파이크** — 프로젝트 최초 인덱싱 시 수 분 소요 가능 (증분 이후 안정화)
- **JSONL 완료 감지 실패(P1)** — PTY 세션에서 응답이 UI 에 반영되지 않는 경우 간헐적 발생

자세한 목록: [CLAUDE.md §5](./CLAUDE.md)

---

## 도움말 / 단축키

앱 내부 `Settings > Help` 패널에 주요 단축키, 기능 요약, 문제 해결 팁이 정리되어 있습니다.

---

## 연락처

- Email: d9ng@outlook.com
- Issues: https://github.com/hang-in/tunaFlow/issues

---

*Private project. 100% AI-authored codebase — Claude Code가 작성, 사람은 방향만 결정합니다.*
