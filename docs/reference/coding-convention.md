---
title: Coding Convention
updated_at: 2026-04-22
canonical: true
status: active
owner: tunaFlow-core
---

# Coding Convention

tunaFlow 코드 작성 규약. CLAUDE.md 에서 분리 — **코드 작성/수정 시작 전에** 읽는다.

## 1. 언어 / 응답

- **한국어 응답**: 사용자 대면 텍스트는 한국어 존댓말.
- **코드 / 경로 / 식별자**: 원문 표기 유지.

## 2. Frontend (React + Zustand)

- **Zustand selector**: broad `useChatStore()` 금지. 개별 `useChatStore((s) => s.field)` 사용 — 불필요한 re-render 방지.
- **Settings 구조**: `settings/` 폴더에 섹션별 분리 파일. `SettingsPanel` 은 thin shell.

## 3. Backend (Tauri + Rust)

- **Tauri command**:
  - 인자는 `camelCase` (serde `rename_all = "camelCase"`).
  - 긴 실행은 `start_*` background 패턴 (즉시 반환 + 이벤트 통지).
  - UI hot-path 의 sync command 는 **`pub async fn` + `spawn_blocking`** 으로 감싼다 (main thread freeze 방지).
- **DB migration**: `add_column_if_missing` 으로 idempotent, 버전 번호 순차 증가.
- **에러 처리**: dev 단계에서 silent fallback 최소화, 명시적 경고/에러 표시.

## 4. 테스트

- **Frontend**: vitest + jsdom.
- **Backend**: `cargo test --lib` (unit) + `cargo test --test db_integration` (integration).
- 새 기능 추가 시 **회귀 테스트 최소 1개** 포함.

## 5. 4-engine parity

- 새 기능 추가 시 Claude / Codex / Gemini / Ollama 4개 엔진 모두에서 동작하는지 확인.
- 모든 엔진이 `build_normalized_prompt_with_budget()` 단일 경로 사용.
- Multi-agent context 전략: `docs/reference/multiAgentContextStrategy.md`.

## 6. Send 함수 패턴

- `runtimeSlice.sendWithEngine(engine)` + `branchSlice.sendThreadMessage()` 모두 `ENGINE_CONFIGS[engine]` 로 command / event 매핑.
- 엔진별 함수 복사 금지.
- 레거시 동기 `send_with_*` 명령은 완전 제거됨.

## 7. 주석 / 코드 스타일

- 기본 no-comment. WHY 가 비자명할 때만 (숨은 제약 / 미묘한 invariant / 특정 bug workaround).
- `현재 task / 호출자 / 이슈 번호` 를 주석에 넣지 않음 (PR 설명에 속함, 코드 evolution 으로 rot).
- 필요 시 1-line 선호. 여러 줄 docstring / multi-paragraph block 은 피한다.
