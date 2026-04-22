<div align="center">

# tunaFlow

**AI Agent Orchestration Client**

[![Tauri 2](https://img.shields.io/badge/Tauri-2.0-FFC131?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![React 18](https://img.shields.io/badge/React-18-61DAFB?logo=react&logoColor=white)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-stable-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![DB Schema](https://img.shields.io/badge/DB_Schema-v44-8b5cf6)](.)
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

## 설계 특징

### Engine Parity — 엔진 전환해도 프롬프트 다시 안 씀
Claude · Codex · Gemini · Ollama 네 엔진이 단일 조립 함수 `build_normalized_prompt_with_budget()` 를 경유합니다. identity · recent context · 장기기억 · 스킬 · 도구 결과가 엔진 무관하게 동일한 ContextPack 으로 조립되므로, 엔진 변경은 프롬프트 재작성이 아니라 한 줄 토글입니다.

### Blind Cross-verification — Plan 결함을 구현 전에 잡음
Plan 은 Architect(Claude Opus) 가 작성하고, 독립된 Reviewer(Codex, blind) 가 `invariant_checks` + 4차원 루브릭(plan_coverage · code_quality · test_coverage · convention) 으로 검증합니다. 설계 단계 BLOCKER 를 라운드 단위로 수렴시켜 구현 비용이 큰 재작업을 줄입니다.

### Branch-adopt 모델 — 대화 트리가 폭증하지 않음
같은 주제를 여러 에이전트에게 **Branch** 로 분기해 실험하고, 결과가 만족스러우면 **adopt** — 요약만 main 대화에 주입합니다. 사이드 분기의 전체 전사가 main 을 오염시키지 않으므로 대화는 결론 흐름만 유지됩니다. Roundtable(RT) 도 이 Branch 의 확장.

### CLI-first — 구독 요금제 내에서 최대치
기본 경로는 Claude Code / Codex / Gemini **CLI**. SDK(API 과금) 는 fallback 으로만 씁니다. 이미 구독 중인 사용자가 추가 토큰 비용 없이 모든 기능을 쓸 수 있도록 설계됐습니다.

---

## 주요 기능

### Orchestration Workflow

Architect → Developer → Reviewer 3-role 시스템.
Plan을 설계하면 Developer가 구현하고, Reviewer가 교차 검증합니다.
실패 시 findings를 분석해 rev.N+1 Plan을 자동 제안합니다.

### Quick / Deep Review

- **Quick**: 단일 Reviewer가 빠르게 검증
- **Deep**: 복수 엔진이 Roundtable로 교차 검증 + 테스트 자동 주입. 4차원 루브릭(plan_coverage · code_quality · test_coverage · convention) + invariant_checks 기반 평가. Reviewer 는 Architect 와 다른 vendor(blind) 로 배정.

### Interactive Session

`-p` 일회성 플래그 없이 CLI 에이전트와 **지속 세션**을 유지합니다. 파일 수정, 명령 실행 등 에이전트의 전체 도구 사용이 가능하며, 세션이 살아있는 한 tunaFlow 가 컨텍스트를 중복 주입하지 않습니다 (claude `--sdk-url` WebSocket 기본 경로 + PTY legacy 폴백).

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
| Claude (Anthropic) | CLI subprocess + WebSocket sdk-session (지속 세션) |
| Codex (OpenAI) | CLI subprocess + app-server (stateful thread) |
| Gemini (Google) | CLI subprocess |
| Ollama / LM Studio / vLLM | HTTP SSE (OpenAI-compatible) |

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
| [Session History](./docs/reference/sessionHistory.md) | 세션별 상세 이력 (최근 설계 결정 추적용) |

---

## 알려진 제약 (Beta)

- **macOS 전용** — Windows/Linux 빌드는 후속 과제
- **ad-hoc 서명** — Gatekeeper 경고 해제 필요 (`xattr -cr /Applications/tunaFlow.app`)
- **RT 중간 스트리밍 미지원** — Roundtable 은 라운드 단위로만 결과 표시
- **최초 인덱싱 지연** — 대규모 프로젝트 최초 1회 수 분 소요 (ONNX 스레드 제한 + 세마포어 + 점진적 인덱싱 적용 후 CPU 스파이크는 완화됨. 증분 이후 안정화)
- **JSONL 완료 감지 실패(P1)** — PTY 세션에서 응답이 UI 에 반영되지 않는 경우 간헐적 발생 (sdk-session WebSocket 경로로 이동 중)

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
