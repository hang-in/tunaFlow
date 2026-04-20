# tunaFlow 베타 전 리팩토링 및 안정화 로드맵

> 생성: 2026-04-20
> 최종 갱신: 2026-04-20
> 성격: execution commitment — 프로덕션급 베타 공개 직전 품질 기준을 고정
> **원칙**: 일정 < 품질. 필요한 시간을 들여 제대로 안정화한다

---

## 0. 스코프 한 줄 요약

베타 공개 전 **5 Phase · 약 16~19 작업일** 규모의 리팩토링 + API 완성 + 품질 보강. 모바일 클라이언트를 베타 범위에 포함하므로 mobile-facing API (`/api/v1/`, Branch detail, rounds aggregate 등) 도 포함된다.

**Non-goals**: 전면 재작성, 신규 feature 추가, 새 엔진 통합, 프로덕션 이후의 최적화 (예: WCAG AA 전체 준수)

---

## 1. 배경

### 1.1 현재 코드베이스 진단 (2026-04-20)

아키텍트 리뷰 6 findings (요약, 상세는 본 문서 2~5절):

- Finding 1: Store slice 가 실질적으로 단일 거대 객체 — cross-slice 직접 write 다수
- Finding 2: `selectProject()` 가 11개 orthogonal concern 을 한 action 에서 수행
- Finding 3: 메인 채팅 / 브랜치 thread / PTY 세 경로의 send pipeline 이 사실상 3중 중복
- Finding 4: UI routing 이 `window` custom event 에 과도 의존 (domain state 도 일부 포함)
- Finding 5: 워크플로우 도메인 규칙 (verdict 집계, subtask auto-complete 등) 이 프런트 헬퍼에 머물러 있음 — 멀티 클라이언트 정합성 위험
- Finding 6: `src-tauri/src/lib.rs` 가 부트스트랩 + 커맨드 레지스트리 집약 — 신규 의존성 추가 시 startup failure surface ↑

테스트는 안정 (Rust 295 unit + 25 integration + FE 222 vitest = 총 542 green). 즉시 전면 재작성 불필요.

### 1.2 베타 공개 기준 (목표)

프로덕션급 품질 요건:
- 사용자 가시 버그 없음
- graceful degradation (subsystem 하나 실패해도 나머지 동작)
- 외부 API 계약 안정화 — URL 버저닝
- 다중 클라이언트 일관성 (desktop + mobile 동일 비즈니스 규칙)
- observability — 사용자 이슈 디버깅 가능
- 접근성 기본선
- 사용자 대면 문서

### 1.3 사용자 결정 (확정)

- **시나리오 C**: Phase A 전체 + 모바일 포함 Deferred API + 비기능 요건 + Phase 4 전체
- **Phase 4-1 (Accessibility)**: 중간 수준 — 스크린 리더 확인 + 포커스 관리 + 색 대비
- **Phase 4-2 (문서)**: 중간 수준 — README + 인앱 Help + 5개 핵심 기능 gif
- **Phase 4-3 (크래시 리포트)**: 간이 — 콘솔 + 파일 로그 강화 + 수동 제출 버튼
- **진행 시점**: 이번 로드맵 확정 직후 새 세션에서 착수. 스케줄 압박 없음
- **핸드오프**: 새 세션이 맥락을 잃지 않도록 철저히 문서화

---

## 2. Phase 1 — Refactoring (6~7일)

Phase 1 은 **프론트엔드 + Rust 내부 구조 정리** 만. 외부 API 계약 변경 없음, 사용자 가시 동작 변경 없음.

### 1-6. lib.rs 부트스트랩 분해 (0.5일) · **가장 먼저**

**순서 근거**: 가장 작은 단위, Rust 에 한정된 변경, 이후 Phase 1 작업의 startup 가드 제공.

- **범위**: `src-tauri/src/lib.rs` `run()` 함수 내 11 단계 초기화 로직을 모듈로 분리
- **도입 파일**:
  - `src-tauri/src/bootstrap/env.rs` — `inherit_shell_path` 및 PATH 확장
  - `src-tauri/src/bootstrap/db.rs` — DB path 결정 + 초기화 + stale cleanup
  - `src-tauri/src/bootstrap/services.rs` — HTTP API 서버, rawq daemon, embedder 초기화, vector backfill
  - `src-tauri/src/bootstrap/window.rs` — 윈도우 상태 복원, orphan process cleanup
- **유지**: `lib.rs` 는 `run()` 진입점 + invoke_handler 등록만 남김
- **완료 기준**:
  - 각 bootstrap 모듈은 `pub fn <name>(app: &AppHandle, ...) -> Result<...>` 형태로 에러 전파
  - startup 실패 시 어느 단계가 실패했는지 로그에서 명확히 구분 가능
  - Rust 테스트 295+25 baseline 유지

### 1-3. send pipeline 통합 (1.5일) · **가장 중요**

**순서 근거**: 가장 큰 drift 원인. 이후 Phase 2 API 작업이 이 위에서 안전하게 진행됨.

- **현재 중복 경로**:
  - `src/stores/slices/runtimeSlice.ts` `sendWithEngine()` — 메인 채팅 경로
  - `src/stores/slices/threadSlice.ts` `sendThreadMessage()` — 브랜치 thread 경로
  - `src/stores/slices/ptyMessageSender.ts` — PTY 경로
- **공통 concern** (세 곳에 반복됨):
  - model fallback (undefined 시 engine/profile 기반 resolution)
  - running queue / messageQueue 관리
  - placeholder 메시지 생성 + 스트림 업데이트
  - event listener setup/cleanup
  - completion 후 DB reload
  - tool-request follow-up hook
- **도입**:
  - `src/lib/sendPipeline/` 디렉토리
  - `sendPipeline(target: { kind: 'main'|'branch'|'pty', convId, branchId? }, prompt, opts)` 공통 실행기
  - 세 기존 경로는 얇은 wrapper 로 축소 (target 만 다름)
- **범위 제외**: UI 렌더링 변경, 엔진 기능 추가, wire format 변경
- **완료 기준**:
  - 메인/브랜치/PTY 가 동일 lifecycle 을 공유
  - 메인만 수정 → 브랜치 자동 반영되는 공통 코드 경로 존재
  - FE 222 baseline 유지, 신규 테스트 추가 권장 (pipeline unit tests)

### 1-5. workflow 도메인 규칙 서비스화 (1일)

**순서 근거**: Phase 2 API 작업의 기반. Rust 전면 이전은 과조이므로 TS 서비스 레이어로 1차 분리.

- **현재 위치**:
  - `src/lib/workflow/branchSync.ts` — verdict 집계 (line 82), subtask auto-complete (line 12)
  - `src/lib/workflow/reviewWorkflow.ts` — review lifecycle 로직
  - `src/components/tunaflow/**` 에 흩뿌려진 도메인 판정 (예: `useSubtaskProgress`)
- **목표**:
  - `src/lib/workflow/services/` 디렉토리 신설
  - 순수 함수 또는 `{getData, transform, persist}` 3-phase 패턴으로 각 규칙 응집
  - UI 컴포넌트는 서비스 호출만 하고 규칙 직접 실행하지 않음
  - HTTP API 핸들러도 동일 서비스를 재사용할 수 있도록 UI-less 경계 유지
- **도메인 규칙 목록** (이전 대상):
  - verdict 집계 (`scanAllReviewerVerdicts`)
  - subtask auto-complete 감지
  - review branch 재사용 판정
  - doom loop 감지
  - file disposition (keep/modify/revert) 파싱
- **Non-goals**: Rust 백엔드로 이전 (post-beta 판단)
- **완료 기준**:
  - `src/lib/workflow/services/` 에 5 이상의 서비스 모듈 존재
  - `branchSync.ts` 가 서비스 호출로 얇아짐
  - 서비스 단위 테스트 추가 (신규 10+ 테스트 기대)

### 1-2. selectProject 분해 (1일)

- **현재**: `src/stores/slices/projectSlice.ts:86` `selectProject()` — 11개 concern
  - 이전 listener cleanup, 상태 초기화, 저장, conversations fetch, Main 자동 생성, profile 기본값, workflow skills load, skills load, skills 추천, stack info refresh, workflow templates, rawq status check + background index, FS watcher, PTY listener
- **목표**:
  - `src/lib/bootstrap/project.ts` 신설
  - 명시적 lifecycle steps:
    1. teardownPreviousProject()
    2. setInitialState(key)
    3. loadConversations(key)
    4. ensureMainConversation(convs)
    5. bootstrapSkills(project)
    6. bootstrapStackInfo(project)
    7. bootstrapRawq(project)
    8. bootstrapFileWatcher(project)
    9. bootstrapPtyListeners(project)
  - 각 step 은 named error 를 throw (ProjectBootstrapError)
  - 상위에서 step 별 로깅 + 실패 시 graceful degradation (예: rawq 실패 시 status=unavailable 로 set 하고 다음 step 계속)
- **완료 기준**:
  - `selectProject()` 는 `runProjectBootstrap(key, callbacks)` 호출만 남음
  - step 별 실패 로그가 독립적으로 구분됨
  - 기존 동작 회귀 없음

### 1-1. Slice 경계 정리 (1일)

- **현재**: threadSlice 가 conversationSlice 의 state (messages, branches, memos, artifacts, conversations) 를 직접 `set()`. `projectSlice` 에서 conversation bootstrap 호출
- **목표**:
  - 각 slice 는 자신의 state 만 write
  - cross-slice 읽기는 OK (`get()`), 쓰기는 **반드시 해당 slice 의 action 을 호출**
  - action 시그니처: `conversationSlice.loadConversation(id): Promise<void>` 등
- **범위**: 6 slice 전체 audit + cross-write 제거 (대략 10~15 곳 예상)
- **완료 기준**:
  - `rg "set\(.+conversations:" src/stores/slices/thread` 가 0 건 (또는 `conversationSlice.*` 호출만 남음)
  - FE 222 baseline 유지

### 1-4. uiRouter slice (1일)

**순서 근거**: Phase 1 마지막. Phase 2 에 영향 없음. 시간 여유에 따라 후순위 가능.

- **현재**: `window.dispatchEvent(new CustomEvent("tunaflow:switch-tab", {...}))` 같은 경로 다수
  - switch-tab, switch-stage, focus-plan, scroll-to-message, open-settings, plan-completed (제거됨) 등
- **분류**:
  - **domain 성격** (ex: focus-plan — 특정 도메인 객체 지시): slice 이전 대상
  - **UI scope** (ex: switch-tab — 로컬 UI 상태): window 이벤트 유지
- **도입**:
  - `src/stores/slices/uiRouterSlice.ts` — `currentTab`, `currentStage`, `focusedPlanId`, `scrollToMessageId`
  - domain 이벤트 이전, UI 이벤트는 일단 유지
- **완료 기준**:
  - domain 이벤트 dispatch 경로 제거
  - `uiRouterSlice` 테스트 추가

---

## 3. Phase 2 — API 완성 (5일)

Phase 2 는 **HTTP API 와 WS 이벤트 계약을 베타 공개 수준으로 완성**. 모바일 δ-Branch / Meta / Workflow 스크린이 베타 범위이므로 해당 API 가 전부 존재해야 한다.

### 2-1. `/api/v1/` 버저닝 도입 (0.5일)

- **목표**: 모든 endpoint 를 `/api/v1/` prefix 로 재라우팅. `/api/` (legacy) 는 deprecation header 첨부해 동시 지원 (1~2 번 release 후 제거)
- **구현**: `src-tauri/src/http_api/mod.rs` 의 `build_router` 에서 동일 handler 를 두 prefix 에 등록, legacy 경로 응답에 `X-API-Deprecated: use /api/v1` 헤더
- **완료 기준**:
  - `curl /api/v1/health` / `/api/health` 둘 다 동작
  - OpenAPI-style 계약 문서 (`docs/api/v1.md`) 초안 작성

### 2-2. `GET /api/v1/branches/{id}` detail (0.5일)

- 현재 리스트만 있음. detail endpoint 추가
- 응답: id, parent conversation id, branch_point_message_id, participants (from rt_config), rounds aggregate (또는 링크), adopted_message_id
- **DB 컬럼 추가**: `branches.adopted_message_id TEXT NULLABLE` — v40 마이그레이션
- 현재 `adopt_branch` 가 parent 에 system msg 를 삽입하지만 branch row 에 id 기록 안 함 → v40 에서 추가

### 2-3. `GET /api/v1/branches/{id}/rounds` aggregate (1일)

- system 헤더 메시지 (`--- Round N · mode · names ---`) 를 파싱해 round 구조로 집계
- 응답: `{ rounds: [{ round_number, started_at, completed_at, participant_responses: [...] }] }`
- 참여자 응답은 해당 round 구간 내 assistant 메시지를 persona 로 그룹

### 2-4. `POST /api/v1/plans/{id}/subtasks/{sid}/status` (0.5일)

- 기존 Tauri 커맨드 `update_subtask_status` 를 HTTP 로 노출
- body: `{ status: "todo"|"in_progress"|"done"|"abandoned", outcome?, updatedBy? }`
- WS 이벤트 `plan:subtask_status_changed` broadcast

### 2-5. `GET /api/v1/conversations/{id}/active-plan` (0.5일)

- `get_active_plan_phase` Rust 로직을 HTTP 로 노출
- 응답: `{ planId, phase, title } | null`

### 2-6. WS event replay / since 커서 (2일) · **가장 복잡**

- **동기**: 모바일 연결 끊김 → 재연결 시 missed events 없으면 UX 깨짐
- **설계**:
  - 신규 테이블 `ws_event_log (id, type, payload, created_at)` — append-only, N시간 TTL cleanup job
  - `/ws/events?since=<epoch_ms>` 쿼리 파라미터 지원 — upgrade 직후 해당 timestamp 이후 로그 replay 전송
  - 모든 `event_tx.send(...)` 경로에서 동시에 log 테이블에 insert (volume 관리 필요)
- **Non-goals**: at-least-once 보증, exactly-once, distributed replay
- **완료 기준**:
  - 연결 끊기고 5분 후 재연결 시 `?since=<5분전>` 로 missed `message:new`/`agent:completed` 이벤트 수신 가능
  - TTL cleanup job 이 1일 이상 오래된 로그 자동 제거

---

## 4. Phase 3 — 프로덕션 비기능 (1.5일)

### 3-1. 에러 메시지 UX 매핑 레이어 (0.5일)

- **현재**: `AppError` 원문이 toast 로 전달되는 경로 있음 — DB raw SQL error, Rust panic 메시지 등 사용자에게 무의미
- **도입**: `src/lib/errors/userFriendlyMessage.ts` — AppError variant → 한국어 사용자 메시지 매핑 테이블
- **Non-goals**: 모든 에러 재분류 (시간 과다) — 대표 20~30 개 variant 만
- **완료 기준**: 주요 플로우(send, plan approve, branch adopt, memory compress)에서 raw error 노출 없음

### 3-2. Observability 감사 (0.5일)

- **감사 대상**: rawq, compression, verdict, startup, meta-trigger, PTY, embedder
- 각 플로우에 INFO/WARN/ERROR 로그가 존재하는지 확인, 공백 있으면 채움
- **도입**: 필요 시 `tracing` crate 추가 검토 (현재는 `eprintln!`). 도입은 베타 후 판단
- **완료 기준**: 위 7 플로우에서 실패 시 원인을 로그만으로 추적 가능

### 3-3. Performance baseline (0.5일)

- **측정 시나리오**:
  - 1000-message 대화 스크롤 60fps 유지 (Virtuoso)
  - compression (Haiku) 요청 → 완료 레이턴시 (p50, p95)
  - 벡터 검색 (memory_semantic) 레이턴시 (5 chunks, 50 chunks, 500 chunks)
- **튜닝**: baseline 미달 시 즉시 해결. 허용 범위면 기록만 하고 넘어감
- **문서**: `docs/reference/performance-baseline.md` 에 측정치 기록

---

## 5. Phase 4 — 접근성 / 문서 / 크래시 (약 4일)

### 4-1. Accessibility 중간 수준 (2일)

- **범위**:
  - 주요 10개 화면 키보드 네비게이션 확인 (Sidebar, CenterPanel 4탭, 드로어, 설정 패널, MetaFloatingChat, Plans, SubtaskReview, Insight, Terminal)
  - Focus ring 표준화 (`:focus-visible` 스타일 통일)
  - 스크린 리더 기본 동작 (ARIA label + landmark)
  - 색 대비 WCAG AA 기본선 확인 (핵심 UI 만)
- **Non-goals**: WCAG AA 전체 준수 (post-beta), 완전 무료 navigator, complex live regions
- **완료 기준**:
  - 10개 화면을 마우스 없이 사용 가능 (기능 완수)
  - axe-core 또는 Lighthouse 감사 점수 70+ (기존보다 개선)

### 4-2. 사용자 문서 중간 수준 (1.5일)

- **범위**:
  - README 전면 개편 (설치, 빠른 시작, 주요 기능, 제한 사항)
  - 인앱 Help 메뉴 — 키보드 단축키, 주요 기능 설명, 문제 해결
  - 5개 핵심 기능 gif (onboarding, plan 생성, branch/RT, Insight, 설정)
- **Non-goals**: 별도 docs site, 튜토리얼 플로우, 다국어
- **완료 기준**: 새 사용자가 문서만 보고 주요 기능 5개 실행 가능

### 4-3. 크래시 리포트 간이 (0.5일)

- **범위**:
  - Rust panic / JS uncaught error 발생 시 `~/.tunaflow/crash-reports/<YYYY-MM-DD-ts>.log` 자동 기록
  - 앱 재시작 시 최근 크래시 있으면 설정 패널에 "이슈 제출" 배지 표시
  - 사용자가 버튼 누르면 로그 첨부 + GitHub issue 생성 링크
- **Non-goals**: Sentry 자동 수집 (privacy), 원격 서버
- **완료 기준**: 크래시 시나리오 수동 시뮬레이션 → 로그 파일 생성 확인 → "이슈 제출" 동작 확인

---

## 6. Phase 5 — Beta readiness gate (2~3일)

- **E2E 시나리오 10~15 수동 검증** (체크리스트화):
  - 프로젝트 첫 생성 → Main 대화 → 첫 메시지
  - Architect 와 Plan 제안 → 승인 → drafting 문서 작성 → subtask 검토 → Dev → Review → Done
  - Branch 생성 → adopt / archive
  - RT 다중 참가자 → verdict 집계
  - Meta inbox 알림 수신 → askMeta
  - Insight 분석 실행 → findings → 상태 업데이트
  - Settings 에서 agent 프로필 생성 → 사용
  - 모바일 client 연결 → HTTP 소비 → WS 구독 → 재연결 복구
  - 긴 대화 (1000+ 메시지) 스크롤 + 검색
  - 앱 재시작 → 세션 이어가기
- **Release notes**: 베타 공개 대상 기능 요약
- **Known issues**: 베타 시점에 알려진 제약 사항 문서
- **Beta announcement draft**: 공개 메시지 초안

---

## 7. 진행 규칙 (MUST)

### 7.1 단일 PR 원칙
- 하나의 Finding / 작업 = 하나의 PR
- 스코프 혼합 금지 (예: Finding 3 하는 PR 에 Finding 1 코드 섞지 않음)
- 각 PR 은 자체로 revertible

### 7.2 테스트 baseline 유지
- Rust 295 unit + 25 integration / FE 222 vitest / TSC 0
- 절대 내려가지 않음. 신규 테스트 추가는 권장
- 각 PR 머지 전 CI 녹색 필수

### 7.3 사용자 가시 동작 변경 없음 (Phase 1 refactoring 기준)
- refactoring Phase 는 외부 관찰 동일해야 함
- 기능 변경이 불가피하면 별도 PR 로 분리 + 사용자 명시 승인

### 7.4 Backup branch
- 큰 변경 전 `backup/<topic>` 로컬 브랜치 생성 (이전 safety 규칙)
- 문제 발생 시 즉시 복구 가능

### 7.5 Stash 보존
- `git stash drop` / `clear` / `pop` **절대 금지**
- 사용자 메모리 규칙 — 과거 유실 사례 있음

### 7.6 순서 준수
- Phase 1 내부 순서는 근거 있음 (1-6 → 1-3 → 1-5 → 1-2 → 1-1 → 1-4)
- 재조정은 사용자 승인 후

### 7.7 HMR 주의
- 작업 중 사용자가 `npm run tauri dev` 돌리고 있을 수 있음
- 대량 파일 변경 (branch switch, merge) 전 사용자 확인 권장

---

## 8. 체크리스트

### Phase 1 (6~7일)
- [ ] 1-6. lib.rs 부트스트랩 분해
- [ ] 1-3. send pipeline 통합
- [ ] 1-5. workflow service layer
- [ ] 1-2. selectProject 분해
- [ ] 1-1. slice 경계 정리
- [ ] 1-4. uiRouter slice

### Phase 2 (5일)
- [ ] 2-1. `/api/v1/` 버저닝 도입
- [ ] 2-2. Branch detail endpoint + v40 migration
- [ ] 2-3. Rounds aggregate endpoint
- [ ] 2-4. Subtask HTTP action
- [ ] 2-5. Active plan pointer
- [ ] 2-6. WS event replay

### Phase 3 (1.5일)
- [ ] 3-1. 에러 메시지 UX 매핑
- [ ] 3-2. Observability 감사
- [ ] 3-3. Performance baseline

### Phase 4 (4일)
- [ ] 4-1. Accessibility 중간
- [ ] 4-2. 문서 중간
- [ ] 4-3. 크래시 리포트 간이

### Phase 5 (2~3일)
- [ ] E2E 시나리오 검증
- [ ] Release notes
- [ ] Known issues
- [ ] Beta announcement draft

---

## 9. Phase 별 완료 기준

### Phase 1 완료 기준
- send pipeline 단일 경로로 수렴 (main/branch/PTY 공통)
- `src/lib/workflow/services/` 레이어 신설 + 5+ 서비스 존재
- `selectProject()` 가 step 기반 lifecycle 로 분해됨 + 단계별 named error
- Slice cross-write 경로 의미 있게 감소 (정량: 현재 10~15 건 → 3 이하)
- `lib.rs` 에 bootstrap concern 남아있지 않음
- domain 성격 window 이벤트 제거 (uiRouter slice 경유)

### Phase 2 완료 기준
- `/api/v1/` 로 모든 endpoint 접근 가능, `/api/` legacy 동시 지원
- 모바일 δ-Branch 구현에 필요한 모든 endpoint 존재
- WS 연결 끊김 복구 (since 커서로 missed events 복구)
- v40 migration 적용 + Branch detail 에 adopted_message_id 반영

### Phase 3 완료 기준
- AppError 원문 toast 노출 경로 0
- 7 플로우 observability 감사 통과
- Performance baseline 문서화 + 허용 범위 달성

### Phase 4 완료 기준
- 10 화면 키보드 네비게이션 + Lighthouse 접근성 70+
- README + 인앱 Help + 5 gif 완성
- 크래시 로그 자동 기록 + 수동 제출 동작 확인

### Phase 5 완료 기준
- 모든 이전 Phase 완료
- E2E 체크리스트 pass
- Release notes / Known issues / Announcement 최종 검토

---

## 10. 비 수행 항목 (명시적 제외)

- 전면 재작성
- 워크플로우 규칙의 Rust 백엔드 완전 이전 (post-beta 판단)
- 모든 `window` 이벤트 제거 (UI-scope 는 유지)
- Sentry 같은 원격 크래시 수집 (privacy)
- WCAG AA 전체 준수 (post-beta)
- 다국어 / 별도 docs site / 튜토리얼 플로우
- BFF 계층 도입 (단일 HTTP API 유지)

이 항목들은 베타 이후 판단한다.

---

## 11. 참조 문서

- `CLAUDE.md` (root) — 프로젝트 개요 + 세션 핸드오프 규칙
- `docs/reference/architecture-detail.md` — 상세 아키텍처
- `docs/api-inquiry-gamma-delta.md` — API 계약 + stability 태그
- `docs/reference/sessionHistory.md` — 과거 세션 이력
- `MEMORY.md` (자동 로드) — 사용자 선호 / 과거 결정 기록
- `docs/plans/refactorRoadmap_handoff_2026-04-20.md` — **새 세션용 핸드오프 (본 로드맵 + 이 문서 둘 다 필수 읽기)**
