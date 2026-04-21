---
title: tunaFlow Beta — Release Notes
updated_at: 2026-04-20
canonical: true
status: draft
owner: tunaFlow-core
---

# tunaFlow Beta — Release Notes

> 버전: **v0.1.0-beta** · 플랫폼: **macOS** · 서명: ad-hoc
> 배포일: 2026-04-XX (E2E 통과 후 확정)

tunaFlow 의 첫 공개 베타입니다. 지난 40여 번의 세션 동안 누적된 기능과, 베타 공개를 앞두고 진행한 5-Phase 리팩토링/하드닝의 결과를 담았습니다.

---

## 하이라이트

### 1. 3-Role Workflow — Architect · Developer · Reviewer

Plan 기반 작업 흐름의 핵심이 완성됐습니다.

- Architect 가 Plan 을 설계하고 drafting 문서를 작성
- Developer 가 구현 Branch 에서 코드 작성
- Reviewer 가 Review Branch 에서 검증 → pass / fail verdict
- 실패 시 findings 를 읽고 rev.N+1 Plan 자동 제안
- Quick / Deep Review 선택 가능 (Deep 은 다엔진 RT + 테스트 자동 주입)

### 2. Branch / Roundtable

- Branch 는 대화 분기 — 드로어 안에서 독립 실험 후 `adopt` 로 부모 대화에 요약 삽입
- Roundtable(RT) 은 Branch 의 확장 모드 — Sequential / Deliberative 두 가지 토론 형식
- RT 전용 페르소나 (`role_guidance`) 가 참가자에게 주입

### 3. ContextPack — 4-Engine Parity

- Claude / Codex / Gemini / Ollama 공통 프롬프트 조립 엔진
- Lite / Standard / Full 자동 Tiering 으로 토큰 예산 동적 배분
- rawq 코드 검색, 장기기억, 실패 학습, 사용자 프로필 주입

### 4. Insight — 프로젝트 품질 분석

- 안정성 · 테스트 · 아키텍처 · 성능 · 보안 · 기술부채 6개 카테고리
- rawq + code-review-graph 가 사전 추출한 데이터를 에이전트가 분석
- `fix_difficulty` 표시, Quick Wins 자동 수정 지원
- Phase I: `tool-request:insight` 마커로 에이전트가 분석 자율 호출

### 5. PTY Terminal

- CLI 에이전트와 `-p` 플래그 없이 인터랙티브 세션 유지
- PTY write queue (FIFO) 로 순서 보장, per-conversation spawn lock 으로 race 방지

### 6. 모바일 클라이언트

- HTTP API (`/api/v1/*` 버저닝) + WebSocket 실시간 이벤트
- `?since=<ms>` 쿼리로 WS 재연결 시 missed event replay
- Claude/Codex/Gemini 세션이 모바일↔데스크톱 간 이어짐
- cloudflared tunnel 로 외부 접속

### 7. UX / 접근성 / 안정성 (Phase 3~4)

- 에러 메시지 한국어 매핑 (AppError 7 variant + 주요 message pattern)
- Focus-visible 표준화 + ARIA landmark (Sidebar/Main/Drawer/Meta)
- Settings > Help 패널 신설 (단축키 · 기능 · 문제 해결 · 크래시 리포트)
- Rust panic hook + JS error hook → `~/.tunaflow/crash-reports/`

---

## 주요 수정 (Phase 1~4, PR #86~#101)

| 영역 | 내용 |
|------|------|
| Refactoring | send pipeline 단일화, slice 경계 정리, workflow 서비스 레이어, lib.rs 부트스트랩 분해, uiRouter slice |
| API | `/api/v1/` 버저닝, Branch detail endpoint (v40), rounds aggregate, subtask status, active plan pointer, WS event replay (v41) |
| Production | 에러 메시지 UX, observability 감사, performance baseline |
| Beta gate | Accessibility, 문서, 크래시 리포트 |

자세한 변경 이력은 [refactorRoadmap_2026-04-20.md](../plans/refactorRoadmap_2026-04-20.md) 와 각 PR 을 참조하세요.

---

## 지원 엔진

| 엔진 | 방식 | 요구사항 |
|------|------|----------|
| Claude (Anthropic) | CLI subprocess | `@anthropic-ai/claude-code` 설치 + 로그인 |
| Codex (OpenAI) | CLI subprocess | `@openai/codex` 설치 + 로그인 |
| Gemini (Google) | CLI subprocess | `@google/gemini-cli` 설치 + 로그인 |
| Ollama / LM Studio / vLLM | HTTP SSE | 로컬 서버 실행 |

---

## 설치 / 업그레이드

### 신규 설치 (macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/hang-in/tunaFlow/main/install.sh | bash
```

ad-hoc 서명이라 Gatekeeper 가 경고를 표시합니다:
```bash
xattr -cr /Applications/tunaFlow.app
```

### 소스 빌드

```bash
git clone https://github.com/hang-in/tunaFlow.git
cd tunaFlow
npm install
npm run tauri dev   # 개발
./scripts/build.sh  # 빌드
```

### 기존 사용자 (세션 33 이후 dev 빌드)

DB 스키마는 v41 로 자동 마이그레이션됩니다. 첫 실행 시 수 초 추가 소요됩니다.

---

## 기술 스택

Tauri 2 + React 18 + TypeScript + Zustand 5 + Tailwind CSS 4 + Rust + SQLite (WAL, v41)

- 코드 검색: rawq sidecar (bge-m3 1024-dim)
- 그래프: code-review-graph
- 외부 연동: HTTP + WS · MCP 서버 `tunaflow-mcp`

---

## 수치 — Beta 시점 baseline

- Rust unit tests: **305**
- Frontend vitest: **293**
- TSC: **0 errors**
- cargo warnings: **0**
- Vec 검색 (11k chunks): 15.5ms (vec0) / 28.7ms (brute-force)
- Long message scroll (1000 msgs): **60fps steady**

---

## 감사의 말

tunaFlow 는 **100% AI 작성** 프로젝트입니다. Claude Code 가 코드를 작성하고, 사람은 방향과 판단만 담당했습니다. 베타까지 이끈 것은 "에이전트가 편해야 결과가 좋아진다" 는 agent-first 철학과, 여러 리팩토링 라운드에서 잘못된 가정을 지적해준 코드 리뷰어 에이전트들입니다.

피드백은 [GitHub Issues](https://github.com/hang-in/tunaFlow/issues) 또는 d9ng@outlook.com 로 부탁드립니다.
