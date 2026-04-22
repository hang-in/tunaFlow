# 코드베이스 리팩토링 제안서 v3

> Status: completed
> Created: 2026-04-12
> Updated: 2026-04-13 (s29 — Tier 1 전체 완료, Tier 2 전체 완료)
> 이전: `codebaseRefactoringProposalV2.md` (v2, 2026-04-04), `codebaseRefactoringProposal.md` (v1, 2026-03-31)

---

## 0. 리팩토링 히스토리

### v1 (2026-03-31)

| 대상 | 제안 | 결과 |
|------|------|------|
| `PlansPanel.tsx` (1,026줄) | plans/ 폴더 9파일 | ✅ 완료 |
| `SettingsPanel.tsx` (904줄) | settings/ 폴더 3파일 | ✅ 완료 |
| `send_common.rs` (1,060줄) | ContextData 구조체 분리 | ✅ 구조체만 |
| `runtimeSlice.ts` (469줄) | sendWithEngine 팩토리 | ✅ 완료 |

### v2 (2026-04-04)

| 대상 | 제안 | 결과 |
|------|------|------|
| `send_common.rs` (1,208줄) | 4파일 분할 | ✅ 완료 |
| `context_pack.rs` (1,172줄) | 5파일 분할 | ✅ 완료 |
| `determine_context_mode()` 추출 | assemble_prompt() 내부 함수 추출 | ✅ 완료 |

### v2 이후 문제: 분할한 파일이 다시 커짐 + 신규 god-file 발생

| 항목 | v2 직후 (2026-04-04) | 현재 (2026-04-12) | 상태 |
|------|---------------------|-------------------|------|
| **총 코드량** | 33,800줄 | **51,348줄** | **+52% (8일 만에)** |
| **500줄+ 파일** | ~12개 | **27개** | **2배 이상** |
| `send_common/ (모듈 합계)` | 분할 직후 ~1,200줄 | **1,940줄** | 분할 의미 퇴색 |
| `vector_search.rs` | 398줄 | **907줄** | +128% |
| `executor.rs` | 451줄 | **968줄** | +115% |

---

## 1. 현재 상태 (2026-04-12)

### 1.1 규모

| 계층 | 줄 수 | v2 대비 |
|------|------|---------|
| Frontend (TS/TSX) | 25,428 | +50% |
| Backend (Rust) | 25,920 | +54% |
| **합계** | **51,348** | **+52%** |
| 테스트 | 406 (Rust 230 + FE 176) | +125% |

### 1.2 500줄+ 파일 전체 목록

**Backend (17파일)**:

| 파일 | 줄 | v2 당시 | 상태 |
|------|---|---------|------|
| `http_api.rs` | **1,162** | 없었음 | 🆕 신규 god-file |
| `pty.rs` | **1,076** | 없었음 | 🆕 신규 god-file |
| `conversation_memory.rs` | **984** | 690 | 📈 +42% |
| `executor.rs` | **968** | 451 | 📈 +115% |
| `vector_search.rs` | **907** | 398 | 📈 +128% |
| `document_index.rs` | **880** | 없었음 | 🆕 신규 |
| `migrations.rs` | **819** | 494 | 📈 (마이그레이션 누적, 정상) |
| `plans.rs` | **781** | 738 | 안정 |
| `skills.rs` | **736** | ~300 | 📈 +145% |
| `embedder.rs` | **724** | 없었음 | 🆕 신규 |
| `rawq.rs` | **708** | 690 | 안정 |
| `projects.rs` | **631** | 383 | 📈 +65% |
| `branches.rs` | **609** | 520 | 📈 +17% |
| `prompt_assembly.rs` | **575** | 530 | 안정 |
| `project_tools.rs` | **563** | 518 | 안정 |
| `send_common/ (모듈 합계)` | **1,940** | 1,208 분할 | 📈 +61% (재성장) |
| `context_pack/ (모듈 합계)` | **1,265** | 1,172 분할 | 안정 |

**Frontend (10파일)**:

| 파일 | 줄 | v2 당시 | 상태 |
|------|---|---------|------|
| `InsightPanel.tsx` | **726** | 없었음 | 🆕 신규 |
| `workflowOrchestration.ts` | **701** | 458 | 📈 +53% |
| `RuntimeSection.tsx` | **647** | 367 | 📈 +76% |
| `PlanCard.tsx` | **632** | 380 | 📈 +66% |
| `insightOrchestration.ts` | **625** | 없었음 | 🆕 신규 |
| `SubtaskReviewView.tsx` | **617** | 493 | 📈 +25% |
| `threadSlice.ts` | **609** | 383 | 📈 +59% |
| `streaming-flow.test.ts` | **592** | 없었음 | 🆕 (테스트, 분할 불필요) |
| `ArtifactsPanel.tsx` | **528** | 395 | 📈 +34% |
| `SkillsPanel.tsx` | **483** | 338 | 📈 +43% |

### 1.3 양호한 부분 (변경하지 않는다)

v1, v2에서 양호했던 부분 + 새로 안정화된 부분:

- `context_pack/ (모듈 합계)` — v2 분할 후 안정 (1,172 → 1,265, +8%)
- `plans/ 폴더` (v1 분할 결과) — 안정
- `settings/ 폴더` (v1 분할 결과) — 안정
- API 레이어 (`src/lib/api/`) — 도메인별 분리 유지
- DB 레이어 (schema/models) — 안정
- `migrations.rs` (819줄) — 마이그레이션 누적은 정상
- `streaming-flow.test.ts` (592줄) — 테스트 파일은 분할 불필요
- `rawq.rs`, `prompt_assembly.rs`, `plans.rs`, `branches.rs` — 안정적 성장

---

## 2. 리팩토링 대상

### Tier 1: 백엔드 신규 god-file 분할 (가장 시급)

#### 2.1 `http_api.rs` (1,162줄) → http_api/ 모듈

모든 HTTP 엔드포인트가 한 파일에 있음. axum 라우터 + 핸들러 + WS + 인증이 혼재.

```
http_api/
├── mod.rs              — Router 조립 + 서버 기동 (~100줄)
├── auth.rs             — Bearer 토큰 인증 미들웨어 (~50줄)
├── conversations.rs    — 대화/메시지 CRUD 엔드포인트 (~200줄)
├── plans.rs            — Plan 승인/거부/상태 엔드포인트 (~150줄)
├── agents.rs           — 에이전트 상태/실행 엔드포인트 (~200줄)
├── state.rs            — /api/state/current + 프로젝트 (~100줄)
└── ws.rs               — WebSocket 이벤트 브릿지 (~150줄)
```

#### 2.2 `pty.rs` (1,076줄) → commands/pty/ 모듈

PTY 전체 로직이 한 파일. 세션 관리 + 메시지 파싱 + 이벤트 처리가 혼재.

```
commands/pty/
├── mod.rs              — Tauri command 진입점 (~100줄)
├── session.rs          — PTY 세션 생성/관리/종료 (~300줄)
├── parser.rs           — 출력 파싱 (JSON/스트림/마커) (~300줄)
└── events.rs           — Tauri 이벤트 발행 + 상태 관리 (~200줄)
```

#### ✅ 2.3 `executor.rs` (968줄) — RT 실행 로직 분할 (s29 완료)

v2 당시 451줄에서 2배 성장. Sequential/Deliberative 실행 + 공통 유틸이 혼재.

```
roundtable_helpers/
├── executor.rs         — 공통: execute_round(), stream_participant (~242줄) ✅
├── types.rs            — 타입 구조체 + 헬퍼 함수 (~273줄) ✅ 신규
├── context.rs          — RtContextCache + RtVectorIndex (~210줄) ✅ 신규
├── sequential.rs       — Sequential 모드 실행 (~250줄) ✅
├── deliberative.rs     — Deliberative 모드 실행 (~250줄) ✅
├── prompt.rs           — 프롬프트 빌더 ✅
└── persist.rs          — DB 저장 ✅
```

#### ✅ 2.4 `conversation_memory.rs` (984줄) — 메모리 압축 분할 (s26 완료)

v2 당시 690줄에서 42% 성장. 압축 + 토픽 파싱 + DB 쿼리 + microcompact가 혼재.

```
commands/
├── conversation_memory.rs  — Tauri command + needs_compression + DB 쿼리 (~300줄)
├── memory_compression.rs   — LLM 호출 + 토픽 파싱 + microcompact (~400줄)
└── memory_topics.rs        — 토픽 구조체 + 포맷팅 + 로드 (~200줄)
```

#### ✅ 2.5 `vector_search.rs` (907줄) — 검색/인덱싱 분리 (s26 완료)

v2 당시 398줄에서 2.3배 성장. document_index.rs(880줄)와 기능 겹침 검토 필요.

```
commands/
├── vector_search.rs        — 검색: search_similar, cosine (~300줄)
├── vector_index.rs         — 대화 인덱싱: index_conversation (~250줄)
├── document_index.rs       — 문서 인덱싱 (기존 유지, ~350줄로 축소 가능)
└── vector_common.rs        — 공통: embedding 직렬화, 청킹 유틸 (~150줄)
```

### Tier 2: 프론트엔드 분할

#### ✅ 2.6 `workflowOrchestration.ts` (701줄) → lib/workflow/ 모듈 (s26 완료)

v2에서 이미 제안됐지만 미실행. 458 → 701줄로 계속 성장 중.

```
lib/workflow/
├── index.ts                — re-export
├── planWorkflow.ts         — syncPlanDocument, requestPlanRevision (~150줄)
├── implementWorkflow.ts    — approveAndStartImplementation, approveImplPlan (~120줄)
├── reviewWorkflow.ts       — startReviewRT, processReviewVerdict (~200줄)
├── reportSync.ts           — syncResultReport, syncReviewReport (~100줄)
└── helpers.ts              — getProjectPath, buildPlanContext (~80줄)
```

#### 2.7 `threadSlice.ts` (609줄) — 스트리밍 로직 추출

v2 당시 383줄에서 59% 성장. runtimeSlice와 스트리밍 로직 중복 (시니어 리뷰 지적사항).

```
stores/slices/
├── threadSlice.ts          — 스레드 상태 + 메시지 관리 (~250줄)
├── streamingUtils.ts       — 공통 스트리밍 로직 (runtimeSlice/threadSlice 공유) (~200줄)
```

#### ✅ 2.8 `InsightPanel.tsx` (726줄) — 분석/표시 분리 (s26 완료)

신규 파일이지만 이미 726줄.

```
context-panel/insight/
├── InsightPanel.tsx        — 패널 셸 + 탭 라우팅 (~150줄)
├── InsightRunner.tsx       — 분석 실행 UI + 프리셋 (~200줄)
├── InsightFindings.tsx     — Findings 목록 + 상세 (~200줄)
└── InsightQuadrant.tsx     — Quadrant 뷰 + 필터 (~150줄)
```

### Tier 3: 후순위

| 파일 | 줄 | 판단 |
|------|---|------|
| `RuntimeSection.tsx` (647) | Settings 항목이 많아서 커진 것. 설정 그룹별 분리 가능하지만 후순위 |
| `PlanCard.tsx` (632) | phase별 UI가 한 컴포넌트. 분할 시 props 전달 복잡해질 수 있음. 후순위 |
| `SubtaskReviewView.tsx` (617) | v2부터 잔존. 수정 시 같이 분할 |
| `ArtifactsPanel.tsx` (528) | 성장 추세 주시. 600줄 넘으면 분할 |
| `SkillsPanel.tsx` (483) | 성장 추세 주시 |
| `skills.rs` (736) | 4-layer 로직이 한 파일. 600줄 넘었으므로 layer별 분리 검토 |
| `projects.rs` (631) | scaffold + CRUD + templates. 기능별 분리 가능 |
| `embedder.rs` (724) | 신규. daemon 관리 + embed 호출이 한 파일. 안정화 후 분할 |

---

## 3. 실행 규칙 (v1, v2와 동일)

1. **pub API 변경 금지** — Tauri command 이름, 함수 시그니처, 반환 타입 유지
2. **한 번에 한 파일** — 여러 파일을 동시에 분할하지 않음
3. **분할 후 즉시 검증** — `cargo check` + `cargo test --lib` + `npx tsc --noEmit` + `npx vitest run`
4. **로직 변경 금지** — 파일 이동만. 버그 수정/기능 추가는 별도 커밋
5. **기존 동작 보존** — 리팩토링 전후 테스트 결과 동일
6. **미래를 위한 추상화 금지** — 지금 필요한 분할만

---

## 4. 실행 계획

### Tier 1 (즉시 — 코더 Opus 컨텍스트 부담 감소 목적)

```
4-1. http_api.rs → http_api/ 모듈 6파일
     검증: cargo check + curl 테스트

4-2. pty.rs → commands/pty/ 모듈 4파일
     검증: cargo check + PTY 동작 확인

4-3. executor.rs → sequential/deliberative 분리
     검증: cargo check + cargo test --lib

4-4. conversation_memory.rs → 3파일 분리
     검증: cargo check + cargo test --lib

4-5. vector_search.rs + document_index.rs 정리
     검증: cargo check + cargo test --lib
```

**예상 변경량**: ~0줄 신규, ~5,000줄 이동. 로직 변경 없음.

### Tier 2 (다음)

```
4-6. workflowOrchestration.ts → lib/workflow/ 5파일
     검증: npx tsc --noEmit + npx vitest run

4-7. threadSlice.ts — streamingUtils.ts 추출 (runtimeSlice 공유)
     검증: npx tsc --noEmit + npx vitest run

4-8. InsightPanel.tsx → insight/ 폴더 4파일
     검증: npx tsc --noEmit
```

### Tier 3 (해당 파일 수정 시 함께)

```
RuntimeSection, PlanCard, SubtaskReviewView, skills.rs, projects.rs
→ 기능 추가/수정 시 같이 분할
```

---

## 5. 검증 체크리스트

```bash
# Backend
cd src-tauri && cargo check
cd src-tauri && cargo test --lib

# Frontend
npx tsc --noEmit
npx vitest run

# HTTP API (Tier 1-1 이후)
curl -s http://localhost:19840/api/state/current -H "Authorization: Bearer $TOKEN"

# PTY (Tier 1-2 이후)
# 앱에서 에이전트 실행 → PTY 스트리밍 정상 확인

# 통합
npm run tauri dev
```

---

## 6. v2 → v3 변화 요약

| 항목 | v2 (2026-04-04) | v3 (2026-04-12) | 변화 |
|------|-----------------|-----------------|------|
| 총 코드량 | 33,800줄 | 51,348줄 | **+52%** |
| 500줄+ 파일 | ~12개 | **27개** | **+125%** |
| 최대 파일 (BE) | send_common 1,208줄 (분할 전) | http_api **1,162줄** (신규) | 신규 god-file |
| 최대 파일 (FE) | SubtaskReviewView 495줄 | InsightPanel **726줄** (신규) | 신규 |
| 해결된 P0 | send_common, context_pack 분할 | — | |
| 신규 P0 | — | http_api, pty, executor | **3개 신규 god-file** |
| 테스트 | 180개 | 406개 | +125% (안전망 강화) |

**핵심 교훈**: v2에서 god-file을 분할했지만, **8일 만에 신규 god-file 3개가 생기고 기존 파일도 다시 커짐.** 기능 추가 속도가 빠르면 리팩토링 주기도 빨라야 합니다. v3 이후에는 **500줄 도달 시 즉시 분할** 규칙을 적용하는 것을 권장합니다.

---

## 7. 제안: 500줄 경고 규칙

v3 이후 god-file 재발 방지를 위해:

```
파일이 500줄을 넘으면:
  → 코더 Opus가 해당 파일 수정 시 "이 파일이 500줄입니다. 분할이 필요합니다" 경고
  → 기능 추가 전에 분할 먼저 수행
  → CLAUDE.md §15 작업 안전 규칙에 추가

예외:
  - migrations.rs (마이그레이션 누적 정상)
  - 테스트 파일 (분할 불필요)
  - types/index.ts (순수 타입 정의)
```

---

## 참고

- v1: `docs/plans/codebaseRefactoringProposal.md` (2026-03-31)
- v2: `docs/plans/codebaseRefactoringProposalV2.md` (2026-04-04)
- 시니어 리뷰 지적: threadSlice/runtimeSlice 스트리밍 로직 중복
- 현재 코드:
  - `src-tauri/src/http_api.rs` (1,162줄)
  - `src-tauri/src/commands/pty.rs` (1,076줄)
  - `src-tauri/src/commands/roundtable_helpers/executor.rs` (968줄)
  - `src-tauri/src/commands/conversation_memory.rs` (984줄)
  - `src-tauri/src/commands/vector_search.rs` (907줄)
  - `src/lib/workflowOrchestration.ts` (701줄)
  - `src/components/tunaflow/context-panel/InsightPanel.tsx` (726줄)
  - `src/stores/slices/threadSlice.ts` (609줄)
