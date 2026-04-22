# i18n 계획 — UI 한/영 분리 + 프롬프트 영어 통일

> Status: ready-to-implement
> Created: 2026-04-14
> Updated: 2026-04-23 (본 세션 신규 컴포넌트 반영 + Invariants + 병렬 허용 + error code 카탈로그)
> 원칙: UI 문자열만 i18n 대상. 에이전트 프롬프트/페르소나는 영어 고정, 응답 언어만 사용자 locale 기반 지시.
> 범위: tunaFlow 앱 내부 문자열. **README 번역 (en/ja/zh) 및 `docs/plans/*.md`, `CLAUDE.md` 같은 개발자 문서는 본 plan scope 외** — README 는 Gemini 수동 번역 경로로 처리.

---

## 0. 현황

| 구분 | 파일 수 | 문자열 수 | 현재 상태 |
|------|---------|-----------|----------|
| 프론트 컴포넌트 | 46 | ~900 (EN+KR 혼재) | 하드코딩, 같은 파일에 한/영 섞임 |
| Stores/Lib | 17 | ~200 | 에러 메시지, 토스트 한/영 혼재 |
| Rust 백엔드 | 25 | ~894 | 도구 스키마 KR, 에러 메시지 KR |
| 에이전트 프롬프트 | ~10 | ~500줄+ | 한국어 (defaultPersonas, insightOrchestration 등) |
| i18n 라이브러리 | — | — | 없음 |

---

## 1. 설계 원칙

### 언어 정책

| 구분 | 언어 | i18n 대상 | 이유 |
|------|------|-----------|------|
| UI 문자열 | 한/영 전환 | O | 사용자 대면 |
| 에이전트 프롬프트 | 영어 고정 | X | LLM 성능 + 토큰 효율 |
| 페르소나 설명 | 영어 고정 | X | 에이전트 지시어 |
| Rust 도구 스키마 description | 영어 고정 | X | 에이전트가 읽는 것 |
| Rust 사용자 대면 에러 | 한/영 전환 | O | 프론트에 표시됨 |
| 응답 언어 | ContextPack 주입 | X | `user_profile.preferredLanguages` 활용 |

### 기술 선택

- **프론트**: `react-i18next` + `i18next`
  - React 생태계 표준 (4M+ weekly downloads)
  - namespace 기반 점진적 적용
  - TypeScript 키 자동완성 지원
  - JSON 번역 파일 → 비개발자 편집 가능
- **백엔드**: Rust i18n 모듈 불필요
  - Rust는 에러 코드만 반환, 프론트에서 i18n 키로 매핑
  - `src-tauri/src/i18n/` 디렉토리 생성하지 않음
- **언어 감지**: 시스템 언어 → Settings에서 수동 변경 가능 → appStore 저장

### 1.1 Rust AppError 코드 카탈로그 (2026-04-23 확정)

`src-tauri/src/errors.rs:32-41` 에 이미 `AppError::code()` 가 존재. Phase 4A-1 은 이 code 를 기반으로 프론트 매핑을 완성:

| Rust variant | code (snake_case) | 프론트 i18n 키 (`error.json`) | 기본 context 전달 |
|---|---|---|---|
| `AppError::Database(_)` | `db_error` | `error.db_error` | SQL / 스키마 원인 요약 |
| `AppError::NotFound(String)` | `not_found` | `error.not_found` | 대상 (e.g. "branch", "plan") |
| `AppError::Io(_)` | `io_error` | `error.io_error` | 파일 경로 / 명령 |
| `AppError::Json(_)` | `json_error` | `error.json_error` | 파싱 위치 |
| `AppError::Agent(String)` | `agent_error` | `error.agent_error` | 엔진 / 원인 |
| `AppError::BadRequest(String)` | `bad_request` | `error.bad_request` | 필드명 / 제약 |
| `AppError::Lock` | `lock_error` | `error.lock_error` | (없음) |

**Serialize 조정** (Phase 4A-1 핵심): 현재 Serialize 구현이 한국어 메시지를 그대로 직렬화하고 있으면, `{ "code": "...", "context": "..." }` 객체 형태로 바꿀 것. `code()` 는 이미 있으므로 variant context 추출 + serialize 만 조정.

```json
// before (예상)
{ "error": "Not found: branch" }

// after
{ "code": "not_found", "context": "branch" }
```

### 1.2 Locale 전달 경로 (Codex 리뷰 반영)

**글로벌 Rust state에 locale을 저장하지 않는다.** 멀티 대화, 브랜치, HTTP API, 백그라운드 작업에서 엉킬 수 있기 때문.

| 경로 | locale 소스 | 방식 |
|------|------------|------|
| **UI 문자열** | 프론트 i18next | 프론트에서 완결, Rust 불필요 |
| **프롬프트 응답 언어** | `user_profile.preferredLanguages` | 이미 per-request로 `assemble_prompt()`에 전달됨 (prompt_assembly.rs:153,180) |
| **Rust 에러 → 프론트 표시** | AppError variant (에러 코드) | Rust는 코드만 반환, 프론트 error.json에서 번역. invoke에 locale 인자 불필요 |

Rust 에러의 경우, 에러 메시지 자체를 다국어로 만드는 대신 **에러 코드를 반환하고 프론트에서 i18n 키로 변환**하는 것이 더 깔끔하다. 현재 `AppError` 계열이 이미 variant별로 구분되어 있으므로, variant → i18n 키 매핑이 가능하다. invoke에 locale 인자를 넘길 필요 없음.

```typescript
// 프론트에서 에러 코드 → 번역
catch (e) {
  const msg = t(`error.${extractErrorCode(e)}`, { fallback: String(e) });
  toast.error(msg);
}
```

---

## 2. 파일 구조

### 프론트엔드 번역 파일

```
src/locales/
  ko/
    common.json       # 버튼(삭제/취소/확인/저장), 상태(로딩/완료/실패), 공통 레이블
    chat.json          # 채팅 입력, 메시지 상태, 토스트
    workflow.json      # Plan/Dev/Review 단계, 마커 UI
    branch.json        # Branch/드로어, adopt, RT
    insight.json       # Insight 패널
    settings.json      # 설정 섹션, 레이블
    sidebar.json       # 사이드바, 검색, 프로젝트
    dialog.json        # 확인/삭제/생성 다이얼로그
    error.json         # Rust AppError code → 사용자 메시지
  en/
    (동일 구조)
  index.ts             # i18n 초기화 (initReactI18next 플러그인)
```

> **Rust 백엔드에 i18n 모듈(`src-tauri/src/i18n/`)은 만들지 않는다.**
> Rust는 에러 코드만 반환하고, 번역은 프론트 `error.json`에서 처리.

### 에이전트 프롬프트 (i18n 아님, 영어 통일)

```
기존 위치 유지:
  src/lib/defaultPersonas.ts     → 영어로 전환
  src/lib/insightOrchestration.ts → 별도 Phase에서 검증 후 전환
  src-tauri/src/commands/agents/tool_handler.rs → 도구 description 영어로 전환
  src-tauri/src/commands/roundtable_helpers/ → 영어로 전환
```

---

## 3. 실행 계획

### Phase 1: 인프라 세팅 (반나절)

```
[1-1] react-i18next + i18next 설치 (10분)
  → npm install react-i18next i18next i18next-browser-languagedetector

[1-2] src/locales/index.ts 초기화 (30분)
  → i18n.use(initReactI18next).init({...})
  → fallbackLng 'en', detection: navigator → appStore 저장
  → namespace: common, chat, workflow, branch, insight, settings, sidebar, dialog, error
  → main.tsx에서 import (I18nextProvider 래핑 불필요 — initReactI18next가 처리)

[1-3] common.json (ko/en) 초기 키 작성 (1시간)
  → 버튼: delete, cancel, confirm, save, close, copy, refresh, search
  → 상태: loading, done, failed, streaming, empty
  → 공통: untitled, noData, error

[1-4] Settings에 언어 선택 UI 추가 (30분)
  → SettingsPanel > General 섹션에 Language 드롭다운
  → appStore에 locale 저장 → i18n.changeLanguage 호출

[1-5] 타입 안전성 설정 (30분)
  → src/types/i18next.d.ts — resources 타입 선언
  → ko/common.json 기준 자동완성 활성화

검증: npm run dev로 언어 전환 동작 확인
```

### Phase 2: 고빈도 UI 영역 (1일)

```
[2-1] chat.json + 채팅 컴포넌트 (2-3시간)
  → ChatPanel, MessageItem, InputBar, useSendActions
  → 입력 placeholder, 토스트, 에러, 상태 메시지

[2-2] settings.json + 설정 컴포넌트 (1-2시간)
  → SettingsPanel 및 하위 섹션 (Profile/Agents/Personas/Skills/Runtime/Terminal)
  → 섹션 타이틀, 레이블, placeholder

[2-3] common.json 확장 + 공통 컴포넌트 (1시간)
  → AppShell, TitleBar, NotificationBell
  → 공통 토스트/확인 메시지

[2-4] sidebar.json + 사이드바 (1-2시간)
  → Sidebar, ProjectOnboardingModal, 우클릭 메뉴
  → 프로젝트/대화/브랜치 관련 레이블

검증: ko/en 전환으로 주요 화면 깨짐 없는지 확인
```

### Phase 3: 나머지 UI (1일)

```
[3-1] workflow.json + 워크플로우 컴포넌트 (2시간)
  → PlanCard, DevProgressView, ApprovalGate
  → PlanProposalCard (designReviewGatePlan 의 2버튼 UI)
  → ArchitectPostReviewPanel (designReviewGatePlan subtask-04)
  → Plan 단계명, 상태, 버튼

[3-2] branch.json + Branch 컴포넌트 (2시간)
  → BranchThreadPanel, CreateRoundtableDialog
  → adopt, archive, RT 관련 메시지

[3-3] insight.json + Insight 컴포넌트 (1시간)
  → InsightPanel 상태 메시지, 진행 로그

[3-4] dialog.json + 다이얼로그 통합 (1시간)
  → 삭제 확인, 생성, SaveArtifactDialog
  → StanceConflictModal (userWorldviewInjectionPlan subtask-03)
  → 모든 confirm/alert 텍스트

[3-5] error.json + Rust 에러 매핑 (1시간)
  → §1.1 카탈로그 7개 code 를 error.json 키로 (db_error, not_found, io_error, json_error, agent_error, bad_request, lock_error)
  → 기존 catch 블록에서 toast.error(String(e)) → toast.error(t(`error.${code}`, { context }))
  → Rust 는 `{ code, context }` JSON 반환, 프론트는 `extractErrorCode(e)` 유틸로 파싱

[3-6] 신규 Settings 컴포넌트 i18n (1시간, settings.json 확장)
  → WorldviewSettings (userWorldviewInjectionPlan subtask-01)
  → SearchSettings (searchPipelineFromSecallPlan-part2 subtask-05)
  → RoleCoveragePanel tentative 배지 (roleAssignmentCoverageUxPlan subtask-02)
  → BackgroundJobStatusItem (userWorldviewInjectionPlan subtask-04)

검증: 전체 화면 ko/en 전환 테스트
```

### Phase 4A: Rust 사용자 대면 문자열 정리 (반나절)

```
[4A-1] Rust 에러 메시지 → 에러 코드 기반 전환 (2시간)
  → AppError variant의 Serialize 출력을 stable error code로 통일
    (예: AppError::NotFound("branch") → { "code": "not_found", "context": "branch" })
  → 한국어 메시지 제거, variant name + context 정보만 반환
  → 프론트 error.json에서 code → 사용자 메시지 매핑

[4A-2] defaultPersonas.ts 영어 전환 (1시간)
  → 페르소나 description, traits, instructions 영어로
  → 사용자 대면 label(페르소나 이름)은 i18n 키로 분리

[4A-3] Rust 도구 스키마 영어 전환 (1시간)
  → tool_handler.rs의 description/parameter 설명 영어로
  → RT prompt 함수 영어로

검증: cargo check + cargo test + vitest
```

### Phase 4B: insightOrchestration 영어 전환 (별도, 검증 필수)

```
[4B-1] insightOrchestration.ts 영어 전환 (2시간)
  → 시스템 프롬프트 영어로 재작성
  → 카테고리 라벨, severity 기준, evidence 설명, JSON 예시 포함
  → 진행 로그 메시지("분석 중...", "완료")는 UI i18n으로 분리

[4B-2] A/B 검증 (2시간)
  → 동일 프로젝트(tunaInsight)에서 한국어 프롬프트 vs 영어 프롬프트 비교
  → 검증 항목:
    - finding 수 + severity 분포 비교
    - JSON 파싱 안정성 (형식 준수율)
    - finding 설명의 구체성/정확성
  → 영어 프롬프트가 동등 이상일 때만 전환 확정
  → 품질 하락 시 한국어 유지 (기능 > 일관성)

[4B-3] ContextPack 응답 언어 지시 확인 (30분)
  → assemble_prompt()에서 user_profile.preferredLanguages 기반 주입 확인
  → 이미 동작 중이면 추가 작업 불필요
  → 미동작 시: "Respond in {lang}." 한 줄 추가

검증: Insight 분석 품질 비교 + 에이전트 응답 언어 확인
```

---

## 4. 주의사항

- **점진적 적용**: 한 번에 전체를 바꾸지 않음. Phase 1에서 인프라를 세우고, Phase 2부터 영역별로 이동
- **키 네이밍**: `namespace.section.action` 패턴 (예: `chat.input.placeholder`, `branch.adopt.confirm`)
- **기존 동작 유지**: 문자열 위치만 이동, 기능 변경 없음
- **fallback**: 키가 없으면 영어 표시 (fallbackLng: 'en')
- **insightOrchestration은 A/B 검증 필수**: 카테고리/severity/evidence가 한국어 기준으로 튜닝되어 있음. 단순 번역이 아니라 분석 품질 변경. 검증 없이 전환하지 않음
- **locale 전달 없음**: Rust에 locale을 넘기지 않음. UI는 프론트 i18next, 프롬프트는 user_profile.preferredLanguages, Rust 에러는 코드만 반환 → 프론트가 번역

---

## 5. 검증

```bash
# Phase 1 검증
npm run dev  # 언어 전환 동작 확인

# Phase 2-3 검증
npx tsc --noEmit       # 타입 체크 (i18n 키 타입 안전성)
npx vitest run         # 기존 테스트 통과
# + 수동: ko/en 전환으로 모든 주요 화면 확인

# Phase 4A 검증
cd src-tauri && cargo check && cargo test --lib
npx tsc --noEmit && npx vitest run

# Phase 4B 검증
# Insight 분석: tunaInsight에서 한국어/영어 프롬프트 각 1회 실행 → 결과 비교
# 에이전트 응답 언어: preferredLanguages 설정 후 실제 대화 확인
```

---

## 6. 하지 않을 것

| 항목 | 이유 |
|------|------|
| 에이전트 프롬프트 다국어 번역 | 번역이 아닌 영어 고정. LLM 성능 + 토큰 효율 |
| Rust 글로벌 locale state | 멀티 대화/브랜치/백그라운드에서 충돌 |
| Rust i18n 모듈 (`src-tauri/src/i18n/`) | 불필요. 에러 코드 기반 프론트 매핑으로 충분 |
| invoke에 locale 인자 전달 | 불필요. Rust는 코드만 반환, 프론트가 번역 |
| 3개 이상 언어 지원 | 당장 ko/en 두 개면 충분. 구조만 확장 가능하게 |
| 런타임 번역 API 연동 | 오프라인 데스크톱 앱, 번역 파일은 빌드 시 포함 |
| SSR/SSG i18n 라우팅 | Tauri SPA, 서버 라우팅 없음 |
| 날짜/숫자 포맷 i18n | 현재 스코프 밖. 필요 시 후속 추가 |
| insightOrchestration 검증 없이 전환 | 분석 품질 변경 위험. A/B 비교 필수 |

---

## 7. 타임라인

```
Phase 1  (반나절):  인프라 + Settings 언어 선택
Phase 2  (1일):     고빈도 UI (chat, settings, sidebar, common)           ┐
Phase 3  (1일):     나머지 UI (workflow, branch, insight, dialog) + 에러 │ Phase 2/3 과
Phase 4A (반나절):  Rust 에러 코드화 + 페르소나/도구 영어 전환             │ 4A 는 독립 파일
Phase 4B (반나절):  insightOrchestration 영어 전환 + A/B 검증             ┘ 영역 — 병렬 OK
──────────────────────────────────────
총: 4일 (단일 Developer 순차) / 3일 (Phase 2-3 와 4A 병렬 시)
```

**병렬 노트** (2026-04-23): Phase 2/3 (TS/TSX 문자열 이동) 과 Phase 4A (Rust `AppError` Serialize + 페르소나/도구 description) 는 건드리는 파일이 완전 독립. 동시 진행 가능. Phase 4B 만은 A/B 검증 결과 의존이라 4A 후.

**PR 전략** (사용자 지시 2026-04-23): 쪼개기 과다 회피 — **PR A: Phase 1~3 + 4A 통짜** (UI i18n + Rust 에러 코드화 + 페르소나 영어), **PR B: Phase 4B 별도** (A/B 검증 결과 rollback 용이성 확보). 총 PR 2개.

---

## 8. Invariants

- **[INV-1]** 에이전트 프롬프트 / 페르소나 설명 / Rust 도구 스키마 description / RT role guidance 는 **영어 고정**. i18n 대상이 아니며 JSON locale 파일에 들어가지 않는다. **이유**: LLM 성능 + 토큰 효율 + 번역 유지보수 부담 회피. **검증**: `defaultPersonas.ts` / `insightOrchestration.ts` / `tool_handler.rs` / `roundtable_helpers/*` grep — 한국어 문자열 잔존 0 (Phase 4A/4B 완료 후).

- **[INV-2]** Rust 측에는 **locale 상태 / 모듈 / invoke 인자** 를 두지 않는다. Rust 는 `AppError::code()` + context 만 반환하고, 번역은 프론트가 수행. 백그라운드 작업 / 멀티 대화 / 브랜치 간 locale 충돌 원천 차단. **이유**: 글로벌 state race. **검증**: `rg "locale|i18n" src-tauri/src` 결과 0 (doc comment 제외).

- **[INV-3]** `insightOrchestration.ts` 영어 전환은 **A/B 검증 (Phase 4B-2) 통과 후에만** merge. 검증 실패 시 한국어 유지. **이유**: 분석 품질 변경 위험 — 단순 번역 아님. **검증**: PR B 의 description 에 A/B 비교 수치 (finding 수 + severity 분포 + JSON 파싱 안정성) 필수 기재.

- **[INV-4]** i18next `fallbackLng = 'en'`. 번역 누락 시 **영어 fallback** 하며 한국어 빈 문자열 / undefined 금지. **이유**: UI 깨짐 방지. **검증**: `i18n.init` 설정에 `fallbackLng: 'en'` + `returnEmptyString: false` 명시.

- **[INV-5]** 번역 키 네이밍은 `namespace.section.action` **3계층** (e.g. `chat.input.placeholder`, `branch.adopt.confirm`). flat key 또는 4+ 계층 금지. **이유**: 자동완성 편의 + 번역가의 탐색 효율. **검증**: locale JSON 스키마 validator 또는 `tsc --noEmit` 시 타입 체크 (src/types/i18next.d.ts resources 타입 선언).

---

## 9. 본 세션 신규 plan 들과의 상호작용

2026-04-14 이후 착수된 plan 들이 신규 UI 컴포넌트를 추가. i18n 은 이들을 **Phase 2/3 에서 흡수**:

| Plan | 신규 컴포넌트 | i18n Phase |
|---|---|---|
| `designReviewGatePlan` | PlanProposalCard (확장), ArchitectPostReviewPanel, RT 드로어 mode 배지 | 3-1 (workflow) |
| `userWorldviewInjectionPlan` | WorldviewSettings, StanceConflictModal, BackgroundJobStatusItem | 3-4 (dialog), 3-6 (settings) |
| `roleAssignmentCoverageUxPlan` | RoleCoveragePanel "제안됨" 배지 | 3-6 (settings) |
| `searchPipelineFromSecallPlan-part2` | SearchSettings | 3-6 (settings) |

**순서 정책** (경로 C): i18n Phase 1 인프라만 먼저 확립. 이후 본 세션 plan 들이 구현될 때 **새 컴포넌트는 처음부터 `t(...)` 기반으로 작성** — i18n-ready 상태로 태어나게 해 Phase 2/3 의 재이동 비용 제거. 기존 컴포넌트만 Phase 2/3 에서 일괄 이동.

---

## 10. Codex 리뷰 반영 사항

### 1차 리뷰

| 지적 | 대응 |
|------|------|
| locale 글로벌 state → 멀티 대화 충돌 | §1.1 추가: per-request 전달, Rust 에러는 에러 코드 기반 프론트 매핑 |
| Phase 4 범위 과소추정 | Phase 4를 4A(Rust 정리) + 4B(프롬프트 전환+검증)로 분리, 총 4일로 조정 |
| insightOrchestration 전환 = 품질 변경 | Phase 4B에 A/B 검증 단계 추가, 검증 실패 시 한국어 유지 |
| I18nextProvider 래핑 불필요 | initReactI18next 플러그인 방식으로 수정, main.tsx에서 import만 |

### 2차 리뷰

| 지적 | 대응 |
|------|------|
| `src-tauri/src/i18n/` 상수 모듈이 에러 코드 방식과 모순 | 삭제. Rust i18n 모듈 만들지 않음 명시 |
| invoke 공통 locale 인자 불필요 | 제거. Rust는 코드만 반환, 프론트가 번역 |
| 4A에서 AppError → stable error code 명시 필요 | 4A-1에 `{ "code": "not_found", "context": "branch" }` 예시 추가 |

---

## 9. 참고

- react-i18next 공식 문서: https://react.i18next.com/
- 현재 프론트 문자열 조사: 88파일, ~1,300+ 문자열
- ContextPack user_profile: `preferredLanguages` 필드 — prompt_assembly.rs:153,180에서 이미 per-request로 읽는 중
- 프로젝트 규칙: projects.rs:481-512에서 "사용자 언어에 맞춰 응답" 이미 명시
- 디자인 시스템: 텍스트 길이 변화에 따른 레이아웃 대응 필요 (EN이 KR보다 보통 20-30% 김)
