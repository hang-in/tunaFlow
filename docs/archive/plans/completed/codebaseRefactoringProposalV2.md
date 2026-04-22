# 코드베이스 리팩토링 제안서 v2

> Status: draft
> Created: 2026-04-03
> 이전 버전: `codebaseRefactoringProposal.md` (v1, 2026-03-31), `scalabilityRefactorPlan.md` (2026-03-26), `opusRefactorPlan.md` (2026-03-26)

---

## 0. 리팩토링 히스토리

### v0 — scalabilityRefactorPlan (2026-03-26, Codex 작성)

| 대상 | 제안 | 결과 |
|------|------|------|
| `chatStore.ts` | slice 분리 | ✅ 6개 slice로 분리 완료 |
| `Sidebar.tsx` | 섹션별 분리 | ✅ ChatsSection, FilesSection 등 분리 |
| `agents.rs` | 공유 함수 추출 | ✅ prepare/finalize 공유, 1168→260줄 |
| `BranchThreadPanel.tsx` | RT/일반 분리 | 부분 완료 (여전히 482줄) |
| `RoundtableView.tsx` | 카드 분리 | ✅ roundtable/ 폴더 3파일 |

### v1 — codebaseRefactoringProposal (2026-03-31)

| 대상 | 제안 | 결과 |
|------|------|------|
| `PlansPanel.tsx` (1,026줄) | plans/ 폴더 9파일 분리 | ✅ 완료 |
| `SettingsPanel.tsx` (904줄) | settings/ 폴더 3파일 | ✅ 완료 |
| `send_common.rs` (1,060줄) | ContextData + load/assemble 분리 | ✅ 구조체 분리 완료, 파일 분할은 미완 |
| `runtimeSlice.ts` (469줄) | sendWithEngine 팩토리 추출 | ✅ 509→311줄 |
| `branchSlice.ts` | ENGINE_CONFIGS 통합 | ✅ 완료 |
| deprecated `isRunning` 제거 | — | ✅ 완료 |

### 잔여 문제 (v1에서 해결 안 된 것)

| 파일 | v1 당시 | 현재 | 상태 |
|------|---------|------|------|
| `send_common.rs` | 1,060줄 | **1,208줄** | 증가. 구조체 분리했으나 파일 분할 미완. 기능 추가로 더 커짐 |
| `context_pack.rs` | ~950줄 | **1,172줄** | 증가. v1에서 미언급. rawq/vector 기능 추가로 커짐 |
| `BranchThreadPanel.tsx` | ~450줄 | **482줄** | 미해결 |
| 에이전트 구현 중복 | 미언급 | 4개×400줄 | 공통 패턴 미추출 |

---

## 1. 현재 상태 (2026-04-03)

### 1.1 코드베이스 규모

| 계층 | 파일 수 | 총 줄 수 | 최대 파일 | v1 대비 |
|------|---------|---------|----------|---------|
| Frontend Components | 68 | ~11,500 | 495줄 | 컴포넌트 수 +16, PlansPanel 분할로 최대 크기 감소 |
| Store Slices | 8 | ~1,600 | 330줄 | 슬라이스 +1, 팩토리 추출로 감소 |
| Lib (utils/api) | ~15 | ~1,500 | 418줄 | 워크플로우/파서/스키마 추가 |
| Backend Commands | ~28 | ~9,500 | 1,208줄 | 파일 수 +8, 총 줄 수 +3,500 (기능 추가) |
| Backend Agents | 12 | ~4,500 | 675줄 | 에이전트 +6 (SDK 어댑터, rawq 확장) |
| DB Layer | 4 | ~1,100 | 483줄 | 마이그레이션 v18→v22 |
| Tests | Frontend 13/96, Rust 60+ | | 테스트 +27 |
| **합계** | **~200** | **~32,100** | | v1 대비 +8,000줄 |

### 1.2 양호한 부분 (변경하지 않는다)

v1에서 양호했던 부분 + 새로 안정화된 부분:

- **API 레이어** (`src/lib/api/`) — 도메인별 분리 유지
- **planProposalParser.ts** — 순수 파서, 테스트 완비
- **DB 레이어** — schema/migrations/models 분리 유지
- **plans/ 폴더** (v1 리팩토링 결과) — 9파일 깔끔 분리
- **settings/ 폴더** (v1 리팩토링 결과) — 3파일 분리
- **roundtable/ 폴더** (v0 리팩토링 결과) — 3파일 분리
- **에러 처리** — AppError enum, unwrap/panic 없음
- **타입 정의** — 순수 타입만, 순환 의존 없음
- **migrations.rs** (483줄) — 마이그레이션은 누적이 정상

---

## 2. 문제 영역

### 2.1 [P0] send_common.rs — 1,208줄, assemble_prompt() 439줄

**v1에서 ContextData 구조체 + load/assemble 분리는 했으나, 파일 분할은 미완.**

현재 함수 구성:

```
send_common.rs (1,208줄)
├── ContextData 구조체           line ~131-168  (38줄)
├── load_context_data()          line ~201-310  (247줄) ← DB 쿼리 12개
├── assemble_prompt()            line ~312-750  (439줄) ← 핵심 문제
├── build_normalized_prompt_with_budget()  line ~752-789  (38줄)
├── prepare_engine_run()         line ~791-831  (41줄)
├── finalize_engine_run()        line ~833-914  (82줄)
├── persist_user_message()       line ~916-950  (35줄)
├── persist_assistant_message()  line ~952-1034 (83줄)
├── persist_assistant_message_with_id()  line ~1036-1168 (133줄)
└── 테스트 4개                   line ~1170-1208 (38줄)
```

**위험**: `assemble_prompt()` 439줄은 auto mode 판정 + 섹션 빌드 + 예산 적용 + 포맷팅을 한 함수에서 수행. 수정 시 사이드이펙트 확인이 어려움.

**제안 분할**:

```
commands/agents_helpers/
├── mod.rs                      — re-export
├── send_common.rs              — build_normalized_prompt_with_budget(), prepare/finalize (얇은 진입점, ~120줄)
├── context_loading.rs          — ContextData, load_context_data() (~290줄)
├── prompt_assembly.rs          — assemble_prompt() + auto mode 로직 (~450줄, 추가 분할 가능)
├── persistence.rs              — persist_user/assistant_message* (~250줄)
├── identity.rs                 — 기존 유지
├── trace_log.rs                — 기존 유지
└── compression.rs              — 기존 유지
```

**검증**: 기존 public API(`build_normalized_prompt_with_budget`, `prepare_engine_run`, `finalize_engine_run`) 시그니처 변경 없음. `agents.rs` 호출부 수정 없음.

### 2.2 [P0] context_pack.rs — 1,172줄, assemble_system_prompt() 275줄

**v1에서 미언급. rawq 확장 + vector search + session discovery 추가로 성장.**

현재 함수 구성:

```
context_pack.rs (1,172줄)
├── ContextPackMeta 구조체       (~20줄)
├── assemble_system_prompt()     (~275줄) ← 핵심 문제
├── build_rawq_section()         (~108줄)
├── build_lite_context_prompt()  (~79줄)
├── build_plan_section()         (~59줄)
├── build_cross_session_section() (~45줄)
├── extract_relevant_skill_sections() (~53줄)
├── 유틸리티 함수 10+개          (~200줄) — jaccard, truncate, fold, dedup 등
└── 테스트                       (~50줄)
```

**제안 분할**:

```
commands/agents_helpers/
├── context_pack/
│   ├── mod.rs                  — ContextPackMeta, mode enum, assemble_system_prompt 진입점 (~100줄)
│   ├── system_prompt.rs        — 섹션 조립 로직 (~200줄)
│   ├── rawq_section.rs         — build_rawq_section (~110줄)
│   ├── section_builders.rs     — plan, cross-session, skills, lite (~200줄)
│   └── utils.rs                — jaccard, truncate, fold, dedup (~200줄)
```

### 2.3 [P1] 프론트엔드 큰 컴포넌트 (400줄+)

| 파일 | 줄 | 분할 방향 |
|------|---|----------|
| `SubtaskReviewView.tsx` (495) | 리뷰 상태 관리 / 리뷰 카드 렌더링 / 액션 버튼 (3파일) |
| `TracePanel.tsx` (483) | 트레이스 목록 / 트레이스 상세 / 필터 (3파일) |
| `BranchThreadPanel.tsx` (482) | v0부터 잔존. 브랜치 헤더 / 메시지 리스트 / RT 분기 (3파일) |
| `DevProgressView.tsx` (466) | 진행 표시 / 리뷰 요청 / Rework UI (3파일) |
| `CenterPanel.tsx` (402) | 탭 라우팅 / 툴바 / 검색 — 허용 범위이나 탭 추가 시 분할 |

### 2.4 [P1] workflowOrchestration.ts — 418줄

**단일 파일에 Plan/Implementation/Review/RT 워크플로우가 혼재.**

제안:

```
lib/workflow/
├── index.ts                    — re-export
├── planWorkflow.ts             — syncPlanDocument, startReviewBranch, requestPlanRevision (~120줄)
├── implementationWorkflow.ts   — approveAndStartImplementation, approveImplPlan (~80줄)
├── reviewWorkflow.ts           — startReviewRT, processReviewVerdict, syncReviewReport (~120줄)
├── reportSync.ts               — syncResultReport, syncReviewReport (~60줄)
└── helpers.ts                  — getProjectPath, buildPlanContext, createAndLinkBranch (~40줄)
```

### 2.5 [P2] Store Slices

| 슬라이스 | 줄 | 문제 | 제안 |
|---------|---|------|------|
| `runtimeSlice.ts` (330) | engine config + agent profile + 런타임 상태 혼재 | engine config 분리 또는 허용 |
| `threadSlice.ts` (318) | 스레드 상태 + 메시지 핸들링 | 300줄대는 허용 범위. 400 넘으면 분할 |

### 2.6 [P2] 에이전트 구현 중복

| 에이전트 | 줄 |
|---------|---|
| `codex.rs` | 456 |
| `openai_compat.rs` | 413 |
| `claude.rs` | 408 |
| `gemini.rs` | 405 |

4개가 유사한 패턴 반복: 바이너리 탐색 → 인자 조립 → subprocess spawn → stdout 파싱 → 결과 구조체.

**제안**: `agent_base.rs`에 공통 패턴 추출 (~150줄). 각 에이전트는 설정 + 파싱만 담당 (~250줄).

다만 **SDK 전환 시 CLI 에이전트가 줄어들 예정**이므로, 지금 추상화하면 곧 삭제할 코드에 투자하는 것. **SDK 전환 이후에 재평가**.

### 2.7 [P3] 커스텀 훅 부재

반복되는 패턴:

| 패턴 | 발생 횟수 | 추출 훅 |
|------|----------|---------|
| dialog open/close state | 10+ | `useDialogState()` |
| invoke() + try/catch + toast | 15+ | `useCommand<T>(name, args)` |
| form input + validation | 5+ | 필요 시 |

---

## 3. 실행 계획

### Tier 1: 백엔드 god-file 분할 (가장 시급)

```
3-1. send_common.rs → 4파일 분할
     - context_loading.rs, prompt_assembly.rs, persistence.rs
     - send_common.rs는 얇은 진입점으로 유지
     검증: cargo check + cargo test --lib

3-2. context_pack.rs → context_pack/ 폴더 4파일
     - mod.rs, system_prompt.rs, rawq_section.rs, section_builders.rs, utils.rs
     검증: cargo check + cargo test --lib
```

**Tier 1.5: assemble_prompt() 내부 함수 추출 (파일 분할 직후)**

```
3-1b. prompt_assembly.rs 내부 — assemble_prompt() 439줄을 하위 함수로 분해
      - determine_context_mode() — auto mode 판정 (~30줄)
      - build_knowledge_sections() — Layer 2-5 섹션 빌드 (~200줄)
      - apply_budget_and_format() — guardrail 적용 + 최종 포맷 (~100줄)
      - assemble_prompt()는 이 3개를 순서대로 호출하는 오케스트레이터 (~50줄)
      검증: cargo check + cargo test --lib (동일 결과)
```

**예상 변경량**: ~0줄 신규, ~2,380줄 이동(파일 분할) + ~50줄 함수 시그니처 추가. 로직 변경 없음.
**리스크**: 낮음. pub API 변경 없음. mod.rs에서 re-export.

### Tier 2: 프론트엔드 컴포넌트 + 워크플로우 분할

```
3-3. workflowOrchestration.ts → lib/workflow/ 5파일
     검증: npx tsc --noEmit + npx vitest run

3-4. SubtaskReviewView.tsx 분할 (495줄 → 3파일)
3-5. BranchThreadPanel.tsx 분할 (482줄 → 3파일)
3-6. DevProgressView.tsx 분할 (466줄 → 3파일)
     검증: npx tsc --noEmit
```

**예상 변경량**: ~0줄 신규, ~1,850줄 이동.
**리스크**: 중간. 컴포넌트 분할 시 props 전달 변경 발생.

### Tier 3: 구조 개선 (필요 시)

```
3-7. useDialogState, useCommand 커스텀 훅 추출
3-8. 에이전트 공통 패턴 추출 (SDK 전환 이후)
3-9. runtimeSlice 분할 (400줄 도달 시)
```

---

## 4. 실행 규칙

v0, v1 리팩토링에서 확립된 원칙 유지:

1. **pub API 변경 금지** — Tauri command 이름, 함수 시그니처, 반환 타입 유지
2. **한 번에 한 파일** — 여러 파일을 동시에 분할하지 않음
3. **분할 후 즉시 검증** — `cargo check` + `cargo test --lib` + `npx tsc --noEmit`
4. **로직 변경 금지** — 파일 이동만. 버그 수정/기능 추가는 별도 커밋
5. **기존 동작 보존** — 리팩토링 전후 테스트 결과 동일
6. **미래를 위한 추상화 금지** — 지금 필요한 분할만. 안 쓸 trait/interface 만들지 않음

---

## 5. 검증 체크리스트

모든 Tier 완료 후:

```bash
# Backend
cd src-tauri && cargo check
cd src-tauri && cargo test --lib

# Frontend
npx tsc --noEmit
npx vitest run

# 통합
npm run tauri dev   # 앱 정상 실행 확인
```

---

## 6. v1 → v2 변화 요약

| 항목 | v1 (2026-03-31) | v2 (2026-04-03) |
|------|-----------------|-----------------|
| 총 코드량 | ~24,000줄 | ~32,100줄 (+34%) |
| 프론트엔드 컴포넌트 | 52개 | 68개 (+31%) |
| 백엔드 파일 | ~30개 | ~83개 (+177%) |
| 최대 파일 (FE) | PlansPanel 1,026줄 | SubtaskReviewView 495줄 (개선) |
| 최대 파일 (BE) | send_common 1,060줄 | send_common 1,208줄 (악화) |
| P0 대상 | PlansPanel, send_common | send_common, context_pack |
| 해결된 P0 | PlansPanel ✅, send_common 구조체만 | — |
| 신규 P0 | — | context_pack (v1 이후 성장) |

**핵심**: 프론트엔드는 v1 리팩토링으로 건강해졌다. 백엔드 2개 파일(send_common, context_pack)이 기능 추가로 계속 커지는 것이 유일한 위험.

---

## 참고

- v0: `docs/plans/scalabilityRefactorPlan.md` (2026-03-26)
- v0: `docs/plans/opusRefactorPlan.md` (2026-03-26)
- v1: `docs/plans/codebaseRefactoringProposal.md` (2026-03-31)
- 현재 코드:
  - `src-tauri/src/commands/agents_helpers/send_common.rs` (1,208줄)
  - `src-tauri/src/commands/agents_helpers/context_pack.rs` (1,172줄)
  - `src/components/tunaflow/context-panel/SubtaskReviewView.tsx` (495줄)
  - `src/components/tunaflow/BranchThreadPanel.tsx` (482줄)
  - `src/lib/workflowOrchestration.ts` (418줄)
